//! The HTTP-agnostic Workload OTA service on a fake NOR flash: capability,
//! prepare/write/finish, staging, activation gate, supersession, isolation.

use std::string::{String, ToString};
use std::vec::Vec;

use iobewi_update_model::{RuntimeApi, Side, WorkloadSupervisor};
use sha2::{Digest as _, Sha256};

use crate::flash::WorkloadFlash;
use crate::layout::{LABEL_META, LABEL_SLOT_A, LABEL_SLOT_B, Region, Unsupported, assemble};
use crate::machine::{Recovery, WorkloadActivator};
use crate::otm2::State;
use crate::service::{Availability, PrepareInput, ServiceError, WorkloadOtaService};
use crate::testing::{ERASE, FakeAccess};

const FLASH: u32 = 0x40_0000;
const API10: RuntimeApi = RuntimeApi::new(1, 0);
const API14: RuntimeApi = RuntimeApi::new(1, 4);

fn block_on<F: core::future::Future>(f: F) -> F::Output {
    let mut f = core::pin::pin!(f);
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    loop {
        if let core::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

#[derive(Default)]
struct Sup(Vec<String>);
impl WorkloadSupervisor for Sup {
    fn switch_to(&mut self, s: Side) {
        self.0.push(std::format!("switch:{s:?}"));
    }
    fn restore(&mut self, s: Side) {
        self.0.push(std::format!("restore:{s:?}"));
    }
}
impl WorkloadActivator for Sup {
    fn stop(&mut self) {
        self.0.push("stop".to_string());
    }
}

fn layout() -> crate::layout::WorkloadLayout {
    // C3 reference layout: 2 x 0x6F000.
    let _ = (LABEL_META, LABEL_SLOT_A, LABEL_SLOT_B);
    assemble(
        Some(Region::new(0x320000, 0x2000)),
        Some(Region::new(0x322000, 0x6F000)),
        Some(Region::new(0x391000, 0x6F000)),
        ERASE,
    )
    .unwrap()
}

fn service(provided: RuntimeApi) -> WorkloadOtaService<FakeAccess> {
    WorkloadOtaService::new(Availability::Supported(WorkloadFlash::new(layout(), FakeAccess::new(FLASH))), provided)
}

fn data(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| (i as u8).wrapping_mul(13).wrapping_add(seed)).collect()
}

fn input(version: &str, bytes: &[u8], requires: RuntimeApi) -> PrepareInput {
    PrepareInput {
        artifact_id: "pod".to_string(),
        version: version.to_string(),
        size: bytes.len() as u64,
        digest: Sha256::digest(bytes).into(),
        requires,
    }
}

/// prepare -> begin -> chunks (16 KiB) -> finish.
fn upload(svc: &WorkloadOtaService<FakeAccess>, version: &str, bytes: &[u8], requires: RuntimeApi) -> Result<crate::service::StagedResult, ServiceError> {
    block_on(async {
        let i = input(version, bytes, requires);
        svc.prepare(&i).await?;
        svc.begin(&i.digest, i.size).await?;
        for piece in bytes.chunks(16 * 1024) {
            assert!(svc.chunk(piece).await);
        }
        svc.finish().await
    })
}

#[test]
fn an_unsupported_device_reports_why_and_refuses_everything() {
    let svc: WorkloadOtaService<FakeAccess> = WorkloadOtaService::new(Availability::Unsupported(Unsupported::MissingMeta), API10);
    let status = block_on(svc.status());
    assert!(!status.supported);
    assert_eq!(status.reason.as_deref(), Some("MissingMeta"));
    let bytes = data(1, 1000);
    assert_eq!(block_on(svc.prepare(&input("1", &bytes, API10))), Err(ServiceError::Unsupported(Unsupported::MissingMeta)));
    assert_eq!(block_on(svc.begin(&[0; 32], 1)), Err(ServiceError::Unsupported(Unsupported::MissingMeta)));
    assert!(matches!(block_on(svc.check_activation(None)), Err(ServiceError::Unsupported(_))));
    assert!(!block_on(svc.chunk(&[1, 2, 3])));
}

#[test]
fn a_verified_upload_becomes_staged_and_status_describes_it_without_slots_being_needed() {
    let svc = service(API10);
    let bytes = data(1, 40_000);
    let staged = upload(&svc, "1.0.0", &bytes, API10).unwrap();
    assert_eq!(staged.written, 40_000);
    assert_eq!(staged.digest, <[u8; 32]>::from(Sha256::digest(&bytes)));
    let status = block_on(svc.status());
    assert!(status.supported && !status.corrupted);
    assert_eq!(status.state, Some(State::Staged));
    let candidate = status.candidate.unwrap();
    assert_eq!((candidate.id.as_str(), candidate.version.as_str(), candidate.size), ("pod", "1.0.0", 40_000));
    assert_eq!(candidate.requires, API10);
    assert_eq!(status.max_artifact_size, 0x6F000);
    assert!(status.active.is_none());
    // Session is clean after staging.
    assert!(!svc.in_progress() && status.prepared == false);
}

#[test]
fn a_wrong_digest_is_never_staged_and_the_active_workload_is_untouched() {
    let svc = service(API10);
    upload(&svc, "1.0", &data(1, 8_000), API10).unwrap();
    block_on(svc.activate(None, &mut Sup::default())).unwrap();
    block_on(svc.storage().unwrap().confirm()).unwrap();
    let before = block_on(svc.status());
    // Prepared for X, streamed Y.
    let good = data(2, 9_000);
    let evil = data(3, 9_000);
    block_on(async {
        let i = input("2.0", &good, API10);
        svc.prepare(&i).await.unwrap();
        svc.begin(&i.digest, i.size).await.unwrap();
        for piece in evil.chunks(16 * 1024) {
            assert!(svc.chunk(piece).await);
        }
        assert!(matches!(svc.finish().await, Err(ServiceError::DigestMismatch(_))));
    });
    let after = block_on(svc.status());
    assert_eq!(after.state, Some(State::Valid));
    assert_eq!(after.candidate, None);
    assert_eq!(after.active, before.active);
}

#[test]
fn oversize_is_refused_at_prepare_before_any_erase() {
    let svc = service(API10);
    let flash_before = svc_flash_snapshot(&svc);
    let mut big = input("1", &data(1, 10), API10);
    big.size = 0x6F000 + 1;
    assert_eq!(block_on(svc.prepare(&big)), Err(ServiceError::TooLarge { max: 0x6F000 }));
    big.size = 0;
    assert_eq!(block_on(svc.prepare(&big)), Err(ServiceError::EmptyArtifact));
    assert_eq!(svc_flash_snapshot(&svc), flash_before, "nothing may be erased or written");
}

fn svc_flash_snapshot(svc: &WorkloadOtaService<FakeAccess>) -> Vec<u8> {
    svc.storage().unwrap().access().0.borrow().data.clone()
}

#[test]
fn begin_requires_a_matching_prepare() {
    let svc = service(API10);
    assert_eq!(block_on(svc.begin(&[1; 32], 10)), Err(ServiceError::NotPrepared));
    let bytes = data(1, 5_000);
    let i = input("1", &bytes, API10);
    block_on(svc.prepare(&i)).unwrap();
    assert_eq!(block_on(svc.begin(&[9; 32], i.size)), Err(ServiceError::SessionMismatch));
    assert_eq!(block_on(svc.begin(&i.digest, i.size + 1)), Err(ServiceError::SessionMismatch));
    assert!(svc.params_match(&i.digest, i.size));
    assert!(!svc.params_match(&[9; 32], i.size));
}

#[test]
fn resume_continues_a_dropped_upload_and_the_final_digest_is_correct() {
    let svc = service(API10);
    let bytes = data(4, 70_000);
    let i = input("1", &bytes, API10);
    block_on(async {
        svc.prepare(&i).await.unwrap();
        svc.begin(&i.digest, i.size).await.unwrap();
        // First three 16 KiB chunks, then "the connection drops".
        for piece in bytes[..3 * 16 * 1024].chunks(16 * 1024) {
            assert!(svc.chunk(piece).await);
        }
    });
    assert!(svc.in_progress());
    // 3 x 16 KiB are 4 KiB multiples: everything is durable, the client resumes there.
    assert_eq!(svc.received(), 3 * 16 * 1024);
    assert_eq!(svc.written(), 3 * 16 * 1024);
    let staged = block_on(async {
        for piece in bytes[3 * 16 * 1024..].chunks(16 * 1024) {
            assert!(svc.chunk(piece).await);
        }
        svc.finish().await
    })
    .unwrap();
    assert_eq!(staged.digest, <[u8; 32]>::from(Sha256::digest(&bytes)));
}

#[test]
fn an_incomplete_stream_is_not_staged() {
    let svc = service(API10);
    let bytes = data(5, 20_000);
    let i = input("1", &bytes, API10);
    block_on(async {
        svc.prepare(&i).await.unwrap();
        svc.begin(&i.digest, i.size).await.unwrap();
        assert!(svc.chunk(&bytes[..8192]).await);
        assert!(matches!(svc.finish().await, Err(ServiceError::Incomplete { .. })));
    });
    assert_eq!(block_on(svc.status()).state, None);
}

#[test]
fn a_staged_candidate_is_superseded_by_a_new_prepare() {
    let svc = service(API10);
    upload(&svc, "1.0", &data(1, 6_000), API10).unwrap();
    upload(&svc, "2.0", &data(2, 7_000), API10).unwrap();
    let status = block_on(svc.status());
    assert_eq!(status.candidate.unwrap().version, "2.0");
    assert_eq!(status.state, Some(State::Staged));
}

#[test]
fn prepare_is_refused_while_a_candidate_is_running_unconfirmed() {
    let svc = service(API10);
    upload(&svc, "1.0", &data(1, 6_000), API10).unwrap();
    block_on(svc.activate(None, &mut Sup::default())).unwrap(); // PendingConfirmation
    let bytes = data(2, 3_000);
    assert!(matches!(block_on(svc.prepare(&input("2.0", &bytes, API10))), Err(ServiceError::Busy(State::PendingConfirmation))));
}

#[test]
fn activation_checks_state_candidate_and_runtime_api_without_persisting() {
    let svc = service(API10);
    // Nothing staged.
    assert!(matches!(block_on(svc.check_activation(None)), Err(ServiceError::WrongState(None))));
    // Stage a Workload that needs 1.4 while the Agent provides 1.0: staging is allowed...
    let bytes = data(7, 9_000);
    upload(&svc, "future", &bytes, API14).unwrap();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    // ...activation is refused, with both versions reported, candidate kept Staged.
    let refused = block_on(svc.check_activation(Some(&digest)));
    assert_eq!(refused, Err(ServiceError::IncompatibleRuntimeApi { required: API14, provided: API10 }));
    let mut sup = Sup::default();
    assert!(matches!(block_on(svc.activate(Some(&digest), &mut sup)), Err(ServiceError::IncompatibleRuntimeApi { .. })));
    assert!(sup.0.is_empty(), "the supervisor must never be called");
    let status = block_on(svc.status());
    assert_eq!(status.state, Some(State::Staged));
    assert_eq!(status.candidate.unwrap().version, "future");
    assert_eq!(block_on(svc.storage().unwrap().recover()).unwrap(), Recovery::Staged { active: None, candidate: Side::A });
}

#[test]
fn a_wrong_candidate_digest_is_refused() {
    let svc = service(API10);
    upload(&svc, "1.0", &data(1, 6_000), API10).unwrap();
    assert_eq!(block_on(svc.check_activation(Some(&[0xAB; 32]))), Err(ServiceError::CandidateMismatch));
}

#[test]
fn check_activation_alone_leaves_the_state_staged_like_a_platform_without_supervisor() {
    let svc = service(API10);
    let bytes = data(1, 6_000);
    upload(&svc, "1.0", &bytes, API10).unwrap();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    // All checks pass...
    assert!(block_on(svc.check_activation(Some(&digest))).is_ok());
    // ...a platform with no supervisor stops here: still Staged, nothing persisted.
    for _ in 0..3 {
        assert!(block_on(svc.check_activation(Some(&digest))).is_ok());
    }
    assert_eq!(block_on(svc.status()).state, Some(State::Staged));
}

#[test]
fn only_a_real_supervisor_activation_moves_the_state() {
    let svc = service(API10);
    let bytes = data(1, 6_000);
    upload(&svc, "1.0", &bytes, API10).unwrap();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let mut sup = Sup::default();
    block_on(svc.activate(Some(&digest), &mut sup)).unwrap();
    assert_eq!(sup.0, ["switch:A"]);
    assert_eq!(block_on(svc.status()).state, Some(State::PendingConfirmation));
}

#[test]
fn workload_operations_never_touch_the_agent_regions() {
    // Everything outside the three Workload regions is a sentinel; a full
    // lifecycle through the service must not change a byte of it.
    let svc = service(API10);
    let l = layout();
    let regions = l.regions();
    {
        let mut guard = svc.storage().unwrap().access().0.borrow_mut();
        let len = guard.data.len();
        for (i, b) in guard.data.iter_mut().enumerate() {
            if !regions.iter().any(|r| (i as u64) >= u64::from(r.offset) && (i as u64) < r.end()) {
                *b = (i % 251) as u8 | 1;
            }
        }
        assert_eq!(len as u32, FLASH);
    }
    let before = svc_flash_snapshot(&svc);
    upload(&svc, "1.0", &data(1, 50_000), API10).unwrap();
    block_on(svc.activate(None, &mut Sup::default())).unwrap();
    block_on(svc.storage().unwrap().confirm()).unwrap();
    upload(&svc, "2.0", &data(2, 60_000), API10).unwrap();
    block_on(svc.activate(None, &mut Sup::default())).unwrap();
    block_on(svc.storage().unwrap().rollback(&mut Sup::default())).unwrap();
    let after = svc_flash_snapshot(&svc);
    for (i, (a, b)) in before.iter().zip(after.iter()).enumerate() {
        if !regions.iter().any(|r| (i as u64) >= u64::from(r.offset) && (i as u64) < r.end()) {
            assert_eq!(a, b, "byte {i:#x} outside the Workload regions changed");
        }
    }
}

// ---------- supersession must not leave a Staged record over a destroyed slot ----------

#[test]
fn starting_a_new_upload_discards_the_old_staged_candidate_before_overwriting_its_slot() {
    let svc = service(API10);
    let old = data(1, 30_000);
    upload(&svc, "old", &old, API10).unwrap();
    assert_eq!(block_on(svc.status()).state, Some(State::Staged));
    // A new artifact is prepared: nothing is overwritten yet, the old candidate still stands...
    let new = data(2, 30_000);
    let i = input("new", &new, API10);
    block_on(svc.prepare(&i)).unwrap();
    assert_eq!(block_on(svc.status()).state, Some(State::Staged));
    // ...but the first byte written lands in the very slot that holds it: from `begin` on
    // the old candidate no longer exists, and a wrong/aborted upload can never leave a
    // Staged record that points at overwritten bytes.
    block_on(svc.begin(&i.digest, i.size)).unwrap();
    let status = block_on(svc.status());
    assert_eq!(status.state, Some(State::Empty));
    assert!(status.candidate.is_none());
    assert!(block_on(svc.chunk(&new[..16 * 1024])));
    // The upload is then abandoned (digest of the wrong bytes, incomplete...): still nothing staged.
    assert!(matches!(block_on(svc.finish()), Err(ServiceError::Incomplete { .. })));
    assert_eq!(block_on(svc.status()).state, Some(State::Empty));
    assert_eq!(
        block_on(svc.storage().unwrap().recover()).unwrap(),
        Recovery::NoWorkload
    );
}

#[test]
fn a_new_upload_never_overwrites_the_rollback_target_of_a_pending_candidate() {
    let svc = service(API10);
    upload(&svc, "1.0", &data(1, 20_000), API10).unwrap();
    block_on(svc.activate(None, &mut Sup::default())).unwrap();
    block_on(svc.storage().unwrap().confirm()).unwrap(); // Valid(A)
    let two = data(2, 20_000);
    let i2 = input("2.0", &two, API10);
    block_on(svc.prepare(&i2)).unwrap(); // reserves B
    // Meanwhile the candidate is activated by someone else: PendingConfirmation (A is the way back).
    upload_to_state_pending(&svc);
    // The stale prepared session must not start writing now.
    assert!(matches!(block_on(svc.begin(&i2.digest, i2.size)), Err(ServiceError::Busy(State::PendingConfirmation))));
}

fn upload_to_state_pending(svc: &WorkloadOtaService<FakeAccess>) {
    // Stage B directly through the storage (bypassing the session) and activate it.
    let storage = svc.storage().unwrap();
    let bytes = data(9, 10_000);
    let req = iobewi_update_model::UpdateRequest::workload(
        iobewi_update_model::ArtifactDescriptor {
            id: "pod".into(),
            version: "other".into(),
            digest: Sha256::digest(&bytes).into(),
            size: bytes.len() as u64,
        },
        API10,
    );
    block_on(async {
        let prepared = storage.prepare(&req).await.unwrap();
        let mut writer = storage.writer(&prepared);
        assert!(writer.append(storage.access(), &bytes).await);
        let committed = writer.finish(storage.access()).await.unwrap();
        storage.commit_staged(&prepared, &committed).await.unwrap();
        storage.activate(&mut Sup::default(), API10).await.unwrap();
    });
}
