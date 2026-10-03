//! NativeRuntime under the real Supervisor, on a fake NOR flash and an in-memory backend.

use std::boxed::Box;
use std::string::ToString;
use std::vec::Vec;

use iobewi_update_model::{RuntimeApi, Side};
use iobewi_workload_ota::flash::WorkloadFlash;
use iobewi_workload_ota::layout::{Region, assemble};
use iobewi_workload_ota::otm2::State;
use iobewi_workload_ota::service::{Availability, PrepareInput, ServiceError, WorkloadOtaService};
use iobewi_workload_ota::supervisor::{BootOutcome, Health, WorkloadSupervisor};
use iobewi_workload_ota::testing::{ERASE, FakeAccess};
use sha2::{Digest as _, Sha256};

use crate::testing::{Behaviour, FakeNative, LAYOUT, image};
use crate::{NativeBackend, NativeRuntime, StopOutcome};

const API10: RuntimeApi = RuntimeApi::new(1, 0);

fn block_on<F: core::future::Future>(f: F) -> F::Output {
    let mut f = core::pin::pin!(f);
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    loop {
        if let core::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

type Svc = WorkloadOtaService<FakeAccess>;
type Sup = WorkloadSupervisor<FakeAccess, NativeRuntime<FakeNative>>;

fn service() -> &'static Svc {
    let layout = assemble(
        Some(Region::new(0x320000, 0x2000)),
        Some(Region::new(0x322000, 0x6F000)),
        Some(Region::new(0x391000, 0x6F000)),
        ERASE,
    )
    .unwrap();
    Box::leak(Box::new(WorkloadOtaService::new(
        Availability::Supported(WorkloadFlash::new(layout, FakeAccess::new(0x40_0000))),
        API10,
    )))
}

fn sup(svc: &'static Svc) -> Sup {
    WorkloadSupervisor::new(svc, NativeRuntime::new(FakeNative::new()))
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn upload(svc: &'static Svc, version: &str, bytes: &[u8], requires: RuntimeApi) {
    block_on(async {
        let i = PrepareInput {
            artifact_id: "native".to_string(),
            version: version.to_string(),
            size: bytes.len() as u64,
            digest: digest(bytes),
            requires,
        };
        svc.prepare(&i).await.unwrap();
        svc.begin(&i.digest, i.size).await.unwrap();
        for piece in bytes.chunks(16 * 1024) {
            assert!(svc.chunk(piece).await);
        }
        svc.finish().await.unwrap();
    });
}

fn state(svc: &'static Svc) -> Option<State> {
    block_on(svc.status()).state
}

fn running(s: &Sup) -> Option<std::string::String> {
    block_on(s.runtime_status()).running.map(|i| i.version)
}

fn activate_and_confirm(s: &Sup, svc: &'static Svc, version: &str, image: &[u8]) {
    upload(svc, version, image, API10);
    block_on(s.activate(&digest(image))).unwrap();
    block_on(s.confirm()).unwrap();
}

#[test]
fn the_image_is_loaded_byte_for_byte_and_started_at_its_entry() {
    let svc = service();
    let img = image(Behaviour::Cooperative, 1, API10);
    let s = sup(svc);
    upload(svc, "1", &img, API10);
    block_on(s.activate(&digest(&img))).unwrap();
    let b = s.runtime().backend();
    assert_eq!(&b.code.borrow()[..], &img[64..64 + 64], "code area == image code");
    assert_eq!(&b.data.borrow()[..], &img[64 + 64..], "data area == image data");
    assert_eq!(b.last_entry.get(), LAYOUT.code_addr + 4);
    assert!(b.executing());
    assert_eq!(state(svc), Some(State::PendingConfirmation));
    assert_eq!(running(&s).as_deref(), Some("1"));
    block_on(s.confirm()).unwrap();
    assert_eq!(state(svc), Some(State::Valid));
}

#[test]
fn stop_is_cooperative_and_really_ends_execution() {
    let svc = service();
    let img = image(Behaviour::Cooperative, 2, API10);
    let s = sup(svc);
    activate_and_confirm(&s, svc, "1", &img);
    block_on(iobewi_workload_ota::supervisor::WorkloadRuntime::stop(s.runtime()));
    assert_eq!(s.runtime().last_stop(), StopOutcome::Cooperative);
    assert!(!s.runtime().backend().executing());
    assert_eq!(running(&s), None);
}

#[test]
fn a_workload_that_ignores_stop_is_halted_after_the_grace_period() {
    let svc = service();
    let img = image(Behaviour::IgnoresStop, 3, API10);
    let s = sup(svc);
    activate_and_confirm(&s, svc, "1", &img);
    let before = s.runtime().backend().now_ms();
    block_on(iobewi_workload_ota::supervisor::WorkloadRuntime::stop(s.runtime()));
    assert_eq!(s.runtime().last_stop(), StopOutcome::Forced, "StopTimeout is reported");
    assert_eq!(s.runtime().forced_stops(), 1);
    assert!(!s.runtime().backend().executing(), "no Workload code runs after stop");
    assert!(s.runtime().backend().now_ms() - before >= 1_500, "the grace period was honoured");
    assert!(s.runtime().backend().halts.get() >= 1);
}

#[test]
fn a_to_b_stops_a_before_the_region_is_rewritten_and_runs_b() {
    let svc = service();
    let a = image(Behaviour::Cooperative, 10, API10);
    let b = image(Behaviour::Cooperative, 20, API10);
    let s = sup(svc);
    activate_and_confirm(&s, svc, "A", &a);
    upload(svc, "B", &b, API10);
    assert_eq!(running(&s).as_deref(), Some("A"), "A keeps running while B is only staged");
    block_on(s.activate(&digest(&b))).unwrap();
    // FakeNative asserts internally that nothing is written while code runs.
    assert_eq!(running(&s).as_deref(), Some("B"));
    assert_eq!(&s.runtime().backend().code.borrow()[..], &b[64..128]);
    assert_eq!(s.runtime().last_stop(), StopOutcome::Cooperative, "A was stopped by the activation");
    block_on(s.confirm()).unwrap();
}

#[test]
fn rollback_really_reloads_the_previous_binary() {
    let svc = service();
    let a = image(Behaviour::Cooperative, 10, API10);
    let b = image(Behaviour::Cooperative, 20, API10);
    let s = sup(svc);
    activate_and_confirm(&s, svc, "A", &a);
    upload(svc, "B", &b, API10);
    block_on(s.activate(&digest(&b))).unwrap();
    assert_eq!(&s.runtime().backend().code.borrow()[..], &b[64..128]);
    block_on(s.rollback()).unwrap();
    assert_eq!(state(svc), Some(State::Valid));
    assert_eq!(running(&s).as_deref(), Some("A"));
    assert_eq!(&s.runtime().backend().code.borrow()[..], &a[64..128], "A's code is back in the region");
    assert_eq!(s.runtime().backend().launches.get(), 3, "A, B, A again");
}

fn hostile(mutate: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut img = image(Behaviour::Cooperative, 5, API10);
    mutate(&mut img);
    img
}

#[test]
fn a_bad_image_refuses_activation_and_nothing_is_loaded_or_executed() {
    type Case = (&'static str, Box<dyn Fn() -> Vec<u8>>, &'static str);
    let cases: Vec<Case> = std::vec![
        ("bad magic", Box::new(|| hostile(|i| i[0] = b'X')), "bad_magic"),
        ("bad format", Box::new(|| hostile(|i| i[4] = 9)), "bad_format_version"),
        ("wrong target (C3 image on S3)", Box::new(|| hostile(|i| i[8] = 2)), "target_mismatch"),
        ("abi mismatch", Box::new(|| hostile(|i| i[10] = 7)), "abi_mismatch"),
        ("future runtime api", Box::new(|| hostile(|i| i[12] = 9)), "runtime_api_mismatch"),
        ("entry outside the code", Box::new(|| hostile(|i| i[24..28].copy_from_slice(&0x4200_0000u32.to_le_bytes()))), "bad_entry"),
        ("entry unaligned", Box::new(|| hostile(|i| i[24..28].copy_from_slice(&(LAYOUT.code_addr + 2).to_le_bytes()))), "bad_entry"),
        ("code size overflow", Box::new(|| hostile(|i| i[36..40].copy_from_slice(&u32::MAX.to_le_bytes()))), "bad_bounds"),
        ("size field lies", Box::new(|| hostile(|i| i[20..24].copy_from_slice(&1u32.to_le_bytes()))), "size_mismatch"),
        ("wrong link address", Box::new(|| hostile(|i| i[28..32].copy_from_slice(&0x403D_0000u32.to_le_bytes()))), "addr_mismatch"),
        ("not an image at all", Box::new(|| std::vec![0xAAu8; 200]), "bad_magic"),
        ("shorter than a header", Box::new(|| std::vec![1u8; 10]), "truncated"),
    ];
    for (name, make, reason) in cases {
        let svc = service();
        let good = image(Behaviour::Cooperative, 1, API10);
        let s = sup(svc);
        activate_and_confirm(&s, svc, "good", &good);
        let (launches, clears) = (s.runtime().backend().launches.get(), s.runtime().backend().clears.get());

        let bad = make();
        upload(svc, "bad", &bad, API10);
        let err = block_on(s.activate(&digest(&bad))).unwrap_err();
        assert_eq!(err, ServiceError::ImageRejected(reason), "{name}");
        assert_eq!(state(svc), Some(State::Staged), "{name}: the candidate stays Staged");
        assert_eq!(running(&s).as_deref(), Some("good"), "{name}: the active Workload is untouched");
        assert!(s.runtime().backend().executing(), "{name}");
        assert_eq!(s.runtime().backend().launches.get(), launches, "{name}: no jump");
        assert_eq!(s.runtime().backend().clears.get(), clears, "{name}: executable memory untouched");
    }
}

#[test]
fn a_header_requiring_another_api_than_ota_recorded_is_refused() {
    let svc = service();
    let img = image(Behaviour::Cooperative, 5, RuntimeApi::new(1, 0));
    let s = sup(svc);
    // OTM2 is told the Workload needs 0.5; the image says 1.0.
    upload(svc, "x", &img, RuntimeApi::new(0, 5));
    assert!(matches!(block_on(s.activate(&digest(&img))), Err(ServiceError::IncompatibleRuntimeApi { .. } | ServiceError::ImageRejected(_))));
    assert_eq!(state(svc), Some(State::Staged));
    assert_eq!(s.runtime().backend().launches.get(), 0);
}

#[test]
fn a_workload_that_never_announces_running_fails_activation_and_rolls_back() {
    let svc = service();
    let a = image(Behaviour::Cooperative, 1, API10);
    let broken = image(Behaviour::NeverRuns, 2, API10);
    let s = sup(svc);
    activate_and_confirm(&s, svc, "A", &a);
    upload(svc, "B", &broken, API10);
    assert_eq!(block_on(s.activate(&digest(&broken))), Err(ServiceError::ActivationFailed));
    assert_eq!(state(svc), Some(State::Valid));
    assert_eq!(running(&s).as_deref(), Some("A"), "A was really restarted");
    assert!(s.runtime().backend().executing());
    assert_eq!(&s.runtime().backend().code.borrow()[..], &a[64..128]);
}

#[test]
fn a_launch_error_is_an_activation_failure_too() {
    let svc = service();
    let a = image(Behaviour::Cooperative, 1, API10);
    let b = image(Behaviour::Cooperative, 2, API10);
    let s = sup(svc);
    activate_and_confirm(&s, svc, "A", &a);
    upload(svc, "B", &b, API10);
    s.runtime().backend().fail_launch.set(true);
    // B fails to launch, and so does the restore of A (the backend is broken): the state
    // stays RollingBack, never an invented Valid.
    assert_eq!(block_on(s.activate(&digest(&b))), Err(ServiceError::RollbackFailed));
    assert_eq!(state(svc), Some(State::RollingBack));
    assert_eq!(running(&s), None);
    s.runtime().backend().fail_launch.set(false);
    block_on(s.rollback()).unwrap();
    assert_eq!(state(svc), Some(State::Valid));
    assert_eq!(running(&s).as_deref(), Some("A"));
}

#[test]
fn health_follows_real_progress() {
    let svc = service();
    let img = image(Behaviour::Freezes, 4, API10);
    let s = sup(svc);
    upload(svc, "F", &img, API10);
    block_on(s.activate(&digest(&img))).unwrap();
    assert_eq!(block_on(s.runtime_status()).health, Health::Healthy, "just started");
    // The Workload stops making progress; the window passes.
    for _ in 0..10 {
        block_on(s.runtime().backend().delay_ms(500));
        s.runtime().sample();
    }
    assert_eq!(block_on(s.runtime_status()).health, Health::Unhealthy);
    assert_eq!(block_on(s.confirm()), Err(ServiceError::Unhealthy(Health::Unhealthy)));
    assert_eq!(state(svc), Some(State::PendingConfirmation));
}

#[test]
fn a_panicking_workload_is_unhealthy_halted_and_cannot_be_confirmed() {
    let svc = service();
    let img = image(Behaviour::Panics, 6, API10);
    let s = sup(svc);
    upload(svc, "P", &img, API10);
    block_on(s.activate(&digest(&img))).unwrap();
    for _ in 0..6 {
        block_on(s.runtime().backend().delay_ms(10));
        s.runtime().sample();
    }
    assert!(!s.runtime().backend().executing(), "a failed Workload stops burning its core");
    assert_eq!(running(&s), None);
    assert_eq!(block_on(s.confirm()), Err(ServiceError::NotRunning));
    block_on(s.rollback()).unwrap();
    assert_ne!(state(svc), Some(State::PendingConfirmation));
}

#[test]
fn boot_restarts_the_valid_workload_from_its_slot_without_any_network() {
    let svc = service();
    let b = image(Behaviour::Cooperative, 20, API10);
    {
        let s = sup(svc);
        activate_and_confirm(&s, svc, "B", &b);
    }
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::Started(Side::A));
    assert_eq!(running(&rebooted).as_deref(), Some("B"));
    assert_eq!(&rebooted.runtime().backend().code.borrow()[..], &b[64..128]);
    assert_eq!(block_on(rebooted.runtime_status()).health, Health::Healthy);
}

#[test]
fn a_reboot_during_probation_rolls_back_to_the_previous_native_workload() {
    let svc = service();
    let a = image(Behaviour::Cooperative, 10, API10);
    let b = image(Behaviour::Cooperative, 20, API10);
    {
        let s = sup(svc);
        activate_and_confirm(&s, svc, "A", &a);
        upload(svc, "B", &b, API10);
        block_on(s.activate(&digest(&b))).unwrap();
        assert_eq!(state(svc), Some(State::PendingConfirmation));
    }
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::RolledBack { restored: Some(Side::A) });
    assert_eq!(running(&rebooted).as_deref(), Some("A"));
    assert_eq!(&rebooted.runtime().backend().code.borrow()[..], &a[64..128]);
}

#[test]
fn a_slot_corrupted_after_staging_is_never_loaded() {
    let svc = service();
    let a = image(Behaviour::Cooperative, 10, API10);
    let b = image(Behaviour::Cooperative, 20, API10);
    let s = sup(svc);
    activate_and_confirm(&s, svc, "A", &a);
    upload(svc, "B", &b, API10);
    // Flip one bit in B's slot behind the OTA's back.
    {
        let layout = *svc.storage().unwrap().layout();
        let mut flash = svc.storage().unwrap().access().0.borrow_mut();
        let at = layout.slot(Side::B).offset as usize + 70;
        flash.data[at] = !flash.data[at];
    }
    let launches = s.runtime().backend().launches.get();
    assert_eq!(block_on(s.activate(&digest(&b))), Err(ServiceError::CandidateCorrupted));
    assert_eq!(s.runtime().backend().launches.get(), launches, "no jump into corrupted code");
    assert_eq!(running(&s).as_deref(), Some("A"));
}
