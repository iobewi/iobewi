//! The Supervisor on a fake NOR flash and a fake runtime: activation, confirmation,
//! rollback, boot reconciliation, crash windows, invariants.

use std::boxed::Box;
use std::string::{String, ToString};
use std::vec::Vec;

use iobewi_update_model::RuntimeApi;
use sha2::{Digest as _, Sha256};

use crate::flash::WorkloadFlash;
use crate::layout::{Region, assemble};
use crate::otm2::State;
use crate::service::{Availability, PrepareInput, ServiceError, WorkloadOtaService};
use crate::supervisor::{BootOutcome, Health, Identity, WorkloadSupervisor};
use crate::testing::{ERASE, FakeAccess, FakeRuntime};

const FLASH: u32 = 0x40_0000;
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

fn service() -> &'static Svc {
    let layout = assemble(
        Some(Region::new(0x320000, 0x2000)),
        Some(Region::new(0x322000, 0x6F000)),
        Some(Region::new(0x391000, 0x6F000)),
        ERASE,
    )
    .unwrap();
    Box::leak(Box::new(WorkloadOtaService::new(
        Availability::Supported(WorkloadFlash::new(layout, FakeAccess::new(FLASH))),
        API10,
    )))
}

fn sup(svc: &'static Svc) -> WorkloadSupervisor<FakeAccess, FakeRuntime> {
    WorkloadSupervisor::new(svc, FakeRuntime::new())
}

fn data(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| (i as u8).wrapping_mul(7).wrapping_add(seed)).collect()
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn upload(svc: &'static Svc, version: &str, bytes: &[u8], requires: RuntimeApi) {
    block_on(async {
        let i = PrepareInput {
            artifact_id: "pod".to_string(),
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

fn running_version(s: &WorkloadSupervisor<FakeAccess, FakeRuntime>) -> Option<String> {
    s.runtime().running_now().map(|i| i.version)
}

/// A confirmed Workload `version` running under a fresh supervisor.
fn valid_workload(svc: &'static Svc, version: &str, bytes: &[u8]) -> WorkloadSupervisor<FakeAccess, FakeRuntime> {
    upload(svc, version, bytes, API10);
    let s = sup(svc);
    block_on(s.activate(&digest(bytes))).unwrap();
    block_on(s.confirm()).unwrap();
    s
}

#[test]
fn activate_runs_the_candidate_and_confirm_makes_it_valid() {
    let svc = service();
    let bytes = data(1, 20_000);
    upload(svc, "1.0", &bytes, API10);
    let s = sup(svc);
    block_on(s.activate(&digest(&bytes))).unwrap();
    assert_eq!(state(svc), Some(State::PendingConfirmation));
    assert_eq!(running_version(&s).as_deref(), Some("1.0"), "PendingConfirmation reflects a really running Workload");
    block_on(s.confirm()).unwrap();
    assert_eq!(state(svc), Some(State::Valid));
    assert_eq!(running_version(&s).as_deref(), Some("1.0"));
    let status = block_on(s.runtime_status());
    assert_eq!(status.health, Health::Healthy);
}

#[test]
fn a_failed_start_rolls_back_to_the_previous_workload_which_runs_again() {
    let svc = service();
    let a = data(1, 12_000);
    let s = valid_workload(svc, "A", &a);
    let b = data(2, 15_000);
    upload(svc, "B", &b, API10);
    s.runtime().fail_start_of(digest(&b));
    assert_eq!(block_on(s.activate(&digest(&b))), Err(ServiceError::ActivationFailed));
    assert_eq!(state(svc), Some(State::Valid));
    let status = block_on(svc.status());
    assert_eq!(status.active.as_ref().map(|x| x.version.as_str()), Some("A"));
    assert_eq!(running_version(&s).as_deref(), Some("A"), "the rollback really restored A");
    assert!(!s.runtime().overlapped());
}

#[test]
fn a_failed_first_activation_ends_empty_with_nothing_running() {
    let svc = service();
    let a = data(3, 9_000);
    upload(svc, "A", &a, API10);
    let s = sup(svc);
    s.runtime().fail_start_of(digest(&a));
    assert_eq!(block_on(s.activate(&digest(&a))), Err(ServiceError::ActivationFailed));
    assert_eq!(state(svc), Some(State::Empty));
    assert!(s.runtime().running_now().is_none());
}

#[test]
fn an_unhealthy_candidate_cannot_be_confirmed_and_manual_rollback_restores_the_previous() {
    let svc = service();
    let s = valid_workload(svc, "A", &data(1, 8_000));
    let b = data(2, 8_000);
    upload(svc, "B", &b, API10);
    block_on(s.activate(&digest(&b))).unwrap();
    assert_eq!(running_version(&s).as_deref(), Some("B"));
    s.runtime().set_health(Some(Health::Unhealthy));
    assert_eq!(block_on(s.confirm()), Err(ServiceError::Unhealthy(Health::Unhealthy)));
    s.runtime().set_health(Some(Health::Unknown));
    assert_eq!(block_on(s.confirm()), Err(ServiceError::Unhealthy(Health::Unknown)));
    assert_eq!(state(svc), Some(State::PendingConfirmation), "a refused confirmation never creates Valid");
    s.runtime().set_health(None);
    block_on(s.rollback()).unwrap();
    assert_eq!(state(svc), Some(State::Valid));
    assert_eq!(running_version(&s).as_deref(), Some("A"));
}

#[test]
fn manual_rollback_without_a_previous_workload_ends_empty() {
    let svc = service();
    let a = data(1, 8_000);
    upload(svc, "A", &a, API10);
    let s = sup(svc);
    block_on(s.activate(&digest(&a))).unwrap();
    block_on(s.rollback()).unwrap();
    assert_eq!(state(svc), Some(State::Empty));
    assert!(s.runtime().running_now().is_none());
}

#[test]
fn confirm_and_rollback_are_refused_in_the_wrong_state() {
    let svc = service();
    let s = sup(svc);
    assert_eq!(block_on(s.confirm()), Err(ServiceError::WrongState(None)));
    assert_eq!(block_on(s.rollback()), Err(ServiceError::WrongState(None)));
    let s = valid_workload(svc, "A", &data(1, 8_000));
    assert_eq!(block_on(s.confirm()), Err(ServiceError::WrongState(Some(State::Valid))));
    assert_eq!(block_on(s.rollback()), Err(ServiceError::WrongState(Some(State::Valid))));
}

#[test]
fn a_candidate_that_is_not_what_is_running_cannot_be_confirmed() {
    let svc = service();
    let b = data(2, 8_000);
    upload(svc, "B", &b, API10);
    let s = sup(svc);
    block_on(s.activate(&digest(&b))).unwrap();
    s.runtime().report_running_as(Some(Identity {
        slot: iobewi_update_model::Side::B,
        id: "pod".into(),
        version: "impostor".into(),
        digest: [9; 32],
        size: 1,
        requires: API10,
    }));
    assert_eq!(block_on(s.confirm()), Err(ServiceError::NotRunning));
    s.runtime().clear_faults();
    block_on(s.confirm()).unwrap();
}

#[test]
fn an_incompatible_runtime_api_is_refused_before_anything_starts_or_persists() {
    let svc = service();
    let b = data(2, 8_000);
    upload(svc, "future", &b, RuntimeApi::new(1, 4));
    let s = sup(svc);
    assert!(matches!(block_on(s.activate(&digest(&b))), Err(ServiceError::IncompatibleRuntimeApi { .. })));
    assert_eq!(state(svc), Some(State::Staged));
    assert!(s.runtime().starts().is_empty() && s.runtime().stops() == 0);
}

#[test]
fn the_slot_digest_is_verified_before_activation_and_a_corrupted_candidate_is_discarded() {
    let svc = service();
    let s = valid_workload(svc, "A", &data(1, 8_000));
    let stops_before = s.runtime().stops();
    let b = data(2, 20_000);
    upload(svc, "B", &b, API10);
    // Bit rot / damage in the staged slot after the write.
    {
        let layout = *svc.storage().unwrap().layout();
        let mut flash = svc.storage().unwrap().access().0.borrow_mut();
        let at = layout.slot(iobewi_update_model::Side::B).offset as usize + 5000;
        flash.data[at] = !flash.data[at];
    }
    assert_eq!(block_on(s.activate(&digest(&b))), Err(ServiceError::CandidateCorrupted));
    assert_eq!(state(svc), Some(State::Valid), "the unusable candidate stopped existing");
    assert_eq!(running_version(&s).as_deref(), Some("A"), "A was never stopped");
    assert_eq!(s.runtime().stops(), stops_before, "the running Workload was not stopped");
}

#[test]
fn the_active_workload_keeps_running_while_the_next_one_is_staged() {
    let svc = service();
    let s = valid_workload(svc, "A", &data(1, 8_000));
    let (starts, stops) = (s.runtime().starts().len(), s.runtime().stops());
    upload(svc, "B", &data(2, 30_000), API10);
    assert_eq!(state(svc), Some(State::Staged));
    assert_eq!((s.runtime().starts().len(), s.runtime().stops()), (starts, stops));
    assert_eq!(running_version(&s).as_deref(), Some("A"));
}

#[test]
fn an_update_switches_a_to_b_stopping_the_old_workload_first() {
    let svc = service();
    let s = valid_workload(svc, "A", &data(1, 8_000));
    let b = data(2, 9_000);
    upload(svc, "B", &b, API10);
    block_on(s.activate(&digest(&b))).unwrap();
    block_on(s.confirm()).unwrap();
    assert_eq!(running_version(&s).as_deref(), Some("B"));
    assert_eq!(state(svc), Some(State::Valid));
    assert!(!s.runtime().overlapped(), "never two Workloads at once");
}

// ---------- boot reconciliation ----------

#[test]
fn boot_valid_starts_the_workload_and_empty_runs_nothing() {
    let svc = service();
    let fresh = sup(svc);
    assert_eq!(block_on(fresh.reconcile_boot()), BootOutcome::Idle);
    assert!(fresh.runtime().running_now().is_none());
    valid_workload(svc, "A", &data(1, 8_000));
    // Reboot: RAM (the runtime) is lost, OTM2 persists.
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::Started(iobewi_update_model::Side::A));
    assert_eq!(running_version(&rebooted).as_deref(), Some("A"));
    assert_eq!(block_on(rebooted.runtime_status()).health, Health::Healthy);
}

#[test]
fn boot_with_a_staged_candidate_runs_the_active_one_and_keeps_the_candidate() {
    let svc = service();
    valid_workload(svc, "A", &data(1, 8_000));
    upload(svc, "B", &data(2, 8_000), API10);
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::Started(iobewi_update_model::Side::A));
    assert_eq!(state(svc), Some(State::Staged));
}

#[test]
fn a_crash_while_activating_rolls_back_at_boot_and_never_assumes_the_candidate_started() {
    let svc = service();
    let a = data(1, 8_000);
    valid_workload(svc, "A", &a);
    let b = data(2, 9_000);
    upload(svc, "B", &b, API10);
    // Power lost right after `Activating` was persisted, before anything started.
    block_on(svc.storage().unwrap().begin_activation(API10)).unwrap();
    assert_eq!(state(svc), Some(State::Activating));
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::RolledBack { restored: Some(iobewi_update_model::Side::A) });
    assert_eq!(state(svc), Some(State::Valid));
    assert_eq!(running_version(&rebooted).as_deref(), Some("A"));
    assert!(rebooted.runtime().starts().iter().all(|i| i.version == "A"), "B was never started");
}

#[test]
fn a_restart_during_probation_is_an_unconfirmed_activation_and_rolls_back() {
    let svc = service();
    valid_workload(svc, "A", &data(1, 8_000));
    let b = data(2, 9_000);
    upload(svc, "B", &b, API10);
    block_on(sup(svc).activate(&digest(&b))).unwrap(); // PendingConfirmation
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::RolledBack { restored: Some(iobewi_update_model::Side::A) });
    assert_eq!(state(svc), Some(State::Valid));
    assert_eq!(running_version(&rebooted).as_deref(), Some("A"));
}

#[test]
fn a_crash_during_rollback_resumes_it_at_boot() {
    let svc = service();
    valid_workload(svc, "A", &data(1, 8_000));
    let b = data(2, 9_000);
    upload(svc, "B", &b, API10);
    block_on(sup(svc).activate(&digest(&b))).unwrap();
    // Rollback intent persisted, then power lost before the restore.
    block_on(svc.storage().unwrap().begin_rollback()).unwrap();
    assert_eq!(state(svc), Some(State::RollingBack));
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::RolledBack { restored: Some(iobewi_update_model::Side::A) });
    assert_eq!(state(svc), Some(State::Valid));
    assert_eq!(running_version(&rebooted).as_deref(), Some("A"));
}

#[test]
fn a_rollback_that_cannot_restore_stays_rolling_back_and_never_invents_valid() {
    let svc = service();
    let a = data(1, 8_000);
    let s = valid_workload(svc, "A", &a);
    let b = data(2, 9_000);
    upload(svc, "B", &b, API10);
    block_on(s.activate(&digest(&b))).unwrap();
    s.runtime().fail_start_of(digest(&a)); // A cannot be restarted
    assert_eq!(block_on(s.rollback()), Err(ServiceError::RollbackFailed));
    assert_eq!(state(svc), Some(State::RollingBack));
    assert!(s.runtime().running_now().is_none());
    // The previous slot and its metadata are untouched: the fault clears, boot completes it.
    let storage = svc.storage().unwrap();
    let record = block_on(storage.record()).unwrap().unwrap();
    assert_eq!(block_on(storage.read_digest(iobewi_update_model::Side::A, 8_000)).unwrap(), digest(&a));
    assert_eq!(record.previous_valid, Some(iobewi_update_model::Side::A));
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::RolledBack { restored: Some(iobewi_update_model::Side::A) });
    assert_eq!(state(svc), Some(State::Valid));
}

#[test]
fn an_unreadable_otm2_runs_nothing_and_invents_nothing() {
    let svc = service();
    valid_workload(svc, "A", &data(1, 8_000));
    for copy in [0usize, 1] {
        let off = svc.storage().unwrap().layout().meta().offset as usize + copy * ERASE as usize + 20;
        let mut flash = svc.storage().unwrap().access().0.borrow_mut();
        flash.data[off] ^= 0xFF;
    }
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::Corrupted);
    assert!(rebooted.runtime().running_now().is_none());
}

#[test]
fn a_valid_slot_that_no_longer_matches_its_digest_is_not_started() {
    let svc = service();
    valid_workload(svc, "A", &data(1, 8_000));
    {
        let layout = *svc.storage().unwrap().layout();
        let mut flash = svc.storage().unwrap().access().0.borrow_mut();
        let at = layout.slot(iobewi_update_model::Side::A).offset as usize + 100;
        flash.data[at] = !flash.data[at];
    }
    let rebooted = sup(svc);
    assert_eq!(block_on(rebooted.reconcile_boot()), BootOutcome::StartFailed);
    assert!(rebooted.runtime().starts().is_empty());
    assert_eq!(state(svc), Some(State::Valid), "state unchanged, nothing invented");
}

// ---------- the invariant, under a power cut at every flash operation ----------

fn scenario(svc: &'static Svc) {
    let s = sup(svc);
    let a = data(1, 9_000);
    let b = data(2, 11_000);
    let _ = (|| -> Result<(), ()> {
        block_on(async {
            for (v, bytes) in [("A", &a), ("B", &b)] {
                let i = PrepareInput {
                    artifact_id: "pod".into(),
                    version: v.into(),
                    size: bytes.len() as u64,
                    digest: digest(bytes),
                    requires: API10,
                };
                svc.prepare(&i).await.map_err(|_| ())?;
                svc.begin(&i.digest, i.size).await.map_err(|_| ())?;
                for piece in bytes.chunks(16 * 1024) {
                    if !svc.chunk(piece).await {
                        return Err(());
                    }
                }
                svc.finish().await.map_err(|_| ())?;
                s.activate(&i.digest).await.map_err(|_| ())?;
                if v == "A" {
                    s.confirm().await.map_err(|_| ())?;
                } else {
                    s.rollback().await.map_err(|_| ())?;
                }
            }
            Ok(())
        })
    })();
}

#[test]
fn after_a_power_cut_anywhere_boot_reconcile_leaves_a_consistent_running_state() {
    // Count the flash operations of the whole scenario.
    let counter = service();
    scenario(counter);
    let total = counter.storage().unwrap().access().0.borrow().ops;
    assert!(total > 20, "scenario too small: {total}");

    for crash in 0..total {
        let svc = service();
        svc.storage().unwrap().access().0.borrow_mut().crash_at = Some(crash);
        scenario(svc);
        svc.storage().unwrap().access().0.borrow_mut().reboot();

        let rebooted = sup(svc);
        let outcome = block_on(rebooted.reconcile_boot());
        let status = block_on(svc.status());
        match outcome {
            BootOutcome::Corrupted => {
                // Only while the very first record is being written.
                assert!(crash < 8, "crash {crash}: corruption must be limited to the first commit");
                assert!(rebooted.runtime().running_now().is_none());
                continue;
            }
            BootOutcome::RollbackFailed | BootOutcome::StartFailed | BootOutcome::Unsupported => {
                panic!("crash {crash}: unexpected {outcome:?}")
            }
            _ => {}
        }
        // Transient states never survive reconciliation.
        assert!(
            matches!(status.state, None | Some(State::Empty) | Some(State::Valid) | Some(State::Staged)),
            "crash {crash}: state {:?} survived boot reconcile",
            status.state
        );
        // The recorded active Workload (if any) is exactly what runs; nothing else does.
        match &status.active {
            Some(active) => {
                let running = rebooted.runtime().running_now().unwrap_or_else(|| panic!("crash {crash}: active but not running"));
                assert_eq!((running.version.as_str(), running.digest), (active.version.as_str(), active.digest), "crash {crash}");
            }
            None => assert!(rebooted.runtime().running_now().is_none(), "crash {crash}: running without an active record"),
        }
        assert!(!rebooted.runtime().overlapped(), "crash {crash}: two Workloads at once");
    }
}
