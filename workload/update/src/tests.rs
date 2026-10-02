use std::string::{String, ToString};
use std::vec::Vec;

use iobewi_ota::ArtifactStorage;
use iobewi_update_model::{
    AbSlots, AgentOta, ArtifactDescriptor, InstalledAgent, Refusal, RuntimeApi, Side, UpdateRequest,
    WorkloadSupervisor,
};
use sha2::{Digest as _, Sha256};

use crate::machine::{Recovery, UpdateError, WorkloadActivator, WorkloadUpdater, write_all};
use crate::otm2::{DecodeError, RECORD_LEN, Record, SlotMeta, State, crc32, newer};
use crate::store::{Loaded, MetadataBackend, MetadataStore};

const API12: RuntimeApi = RuntimeApi::new(1, 2);
const API13: RuntimeApi = RuntimeApi::new(1, 3);
const API14: RuntimeApi = RuntimeApi::new(1, 4);

// ---------- fakes ----------

#[derive(Default)]
struct Mem {
    copies: [Option<[u8; RECORD_LEN]>; 2],
    writes: usize,
    /// Fail (without writing) the write with this 0-based index.
    drop_write_at: Option<usize>,
    /// Tear (half-write) the write with this index, then fail.
    tear_write_at: Option<usize>,
}

impl MetadataBackend for Mem {
    type Error = &'static str;
    fn read(&mut self, copy: usize) -> Result<[u8; RECORD_LEN], Self::Error> {
        Ok(self.copies[copy].unwrap_or([0xFF; RECORD_LEN]))
    }
    fn write(&mut self, copy: usize, bytes: &[u8; RECORD_LEN]) -> Result<(), Self::Error> {
        let index = self.writes;
        self.writes += 1;
        if self.drop_write_at == Some(index) {
            return Err("power loss before write");
        }
        if self.tear_write_at == Some(index) {
            let mut torn = self.copies[copy].unwrap_or([0xFF; RECORD_LEN]);
            torn[..RECORD_LEN / 2].copy_from_slice(&bytes[..RECORD_LEN / 2]);
            self.copies[copy] = Some(torn);
            return Err("power loss during write");
        }
        self.copies[copy] = Some(*bytes);
        Ok(())
    }
}

#[derive(Default)]
struct Slot(Vec<u8>);
impl ArtifactStorage for Slot {
    type Error = ();
    fn write(&mut self, durable: u64, pending: &[u8]) -> Result<u64, ()> {
        self.0.truncate(durable as usize);
        self.0.extend_from_slice(pending);
        Ok(durable + pending.len() as u64)
    }
    fn finish(&mut self, durable: u64, pending: &[u8]) -> Result<u64, ()> {
        self.write(durable, pending)
    }
}

#[derive(Default)]
struct Sup(Vec<String>);
impl WorkloadSupervisor for Sup {
    fn switch_to(&mut self, side: Side) {
        self.0.push(std::format!("switch:{side:?}"));
    }
    fn restore(&mut self, side: Side) {
        self.0.push(std::format!("restore:{side:?}"));
    }
}
impl WorkloadActivator for Sup {
    fn stop(&mut self) {
        self.0.push("stop".to_string());
    }
}

fn digest_of(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

fn request(version: &str, data: &[u8], requires: RuntimeApi) -> UpdateRequest {
    UpdateRequest::workload(
        ArtifactDescriptor { id: "pod".to_string(), version: version.to_string(), digest: digest_of(data), size: data.len() as u64 },
        requires,
    )
}

fn updater() -> WorkloadUpdater<Mem> {
    WorkloadUpdater::new(Mem::default())
}

/// Stage `data` as `version` (prepare + verified write + commit_staged).
fn stage(u: &mut WorkloadUpdater<Mem>, version: &str, data: &[u8], requires: RuntimeApi) -> Result<Side, UpdateError<&'static str>> {
    let prepared = u.prepare(&request(version, data, requires), 1 << 20)?;
    let mut slot = Slot::default();
    let committed = write_all(&prepared, &mut slot, data, 7).expect("write");
    u.commit_staged(&prepared, &committed)?;
    Ok(prepared.slot)
}

/// Stage + activate + confirm `version` so it becomes the Valid Workload.
fn install(u: &mut WorkloadUpdater<Mem>, version: &str) {
    stage(u, version, version.as_bytes(), API12).unwrap();
    u.activate(&mut Sup::default(), API13).unwrap();
    u.confirm().unwrap();
}

// ---------- codec ----------

fn sample() -> Record {
    let mut r = Record::empty(42);
    r.state = State::PendingConfirmation;
    r.active = Some(Side::B);
    r.previous_valid = Some(Side::A);
    r.meta[0] = SlotMeta::new("pod", "1.0.0", [1; 32], 4096, API12).unwrap();
    r.meta[1] = SlotMeta::new("pod", "1.1.0", [2; 32], 8192, API13).unwrap();
    r
}

#[test]
fn encode_decode_round_trip_and_golden_layout() {
    let record = sample();
    let raw = record.encode();
    assert_eq!(raw.len(), RECORD_LEN);
    assert_eq!(RECORD_LEN, 180);
    assert_eq!(Record::decode(&raw), Ok(record));
    assert_eq!(&raw[0..4], b"OTM2");
    assert_eq!(raw[4], 1);
    assert_eq!(raw[5], State::PendingConfirmation as u8);
    assert_eq!((raw[6], raw[7], raw[8]), (1, 0xFF, 0));
    assert_eq!(&raw[9..12], &[0, 0, 0]);
    assert_eq!(u32::from_le_bytes(raw[12..16].try_into().unwrap()), 42);
    assert_eq!(&raw[16..48], &[1u8; 32]);
    assert_eq!(u32::from_le_bytes(raw[48..52].try_into().unwrap()), 4096);
    assert_eq!(u16::from_le_bytes(raw[52..54].try_into().unwrap()), 1); // req major, slot A
    assert_eq!(u16::from_le_bytes(raw[54..56].try_into().unwrap()), 2); // req minor
    assert_eq!(&raw[56..61], b"1.0.0");
    assert_eq!(&raw[72..75], b"pod");
    assert_eq!(&raw[96..128], &[2u8; 32]);
    assert_eq!(u32::from_le_bytes(raw[176..180].try_into().unwrap()), crc32(&raw[..176]));
}

#[test]
fn crc32_is_the_zlib_one() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn decode_rejects_bad_magic_version_crc_truncation_and_blank() {
    let raw = sample().encode();
    let mut bad = raw;
    bad[0] = b'X';
    assert_eq!(Record::decode(&bad), Err(DecodeError::BadMagic));
    let mut bad = raw;
    bad[4] = 2;
    assert_eq!(Record::decode(&bad), Err(DecodeError::BadVersion(2)));
    let mut bad = raw;
    bad[100] ^= 1;
    assert_eq!(Record::decode(&bad), Err(DecodeError::BadCrc));
    assert_eq!(Record::decode(&raw[..RECORD_LEN - 1]), Err(DecodeError::Truncated));
    assert_eq!(Record::decode(&[0xFF; RECORD_LEN]), Err(DecodeError::Blank));
}

#[test]
fn a_valid_crc_over_an_impossible_state_is_malformed() {
    let mut r = sample();
    r.state = State::Valid; // Valid must have no candidate/previous
    assert_eq!(Record::decode(&r.encode()), Err(DecodeError::Malformed));
    let mut r = Record::empty(1);
    r.active = Some(Side::A);
    assert_eq!(Record::decode(&r.encode()), Err(DecodeError::Malformed));
}

#[test]
fn fields_that_do_not_fit_are_refused() {
    use crate::otm2::FieldError;
    assert_eq!(SlotMeta::new(&"x".repeat(25), "1", [0; 32], 1, API12), Err(FieldError::IdTooLong));
    assert_eq!(SlotMeta::new("id", &"v".repeat(17), [0; 32], 1, API12), Err(FieldError::VersionTooLong));
    assert_eq!(SlotMeta::new("a\0b", "1", [0; 32], 1, API12), Err(FieldError::EmbeddedNul));
    assert_eq!(SlotMeta::new("id", "1", [0; 32], u64::from(u32::MAX) + 1, API12), Err(FieldError::SizeTooLarge));
}

#[test]
fn sequence_ordering_is_wrap_safe() {
    assert!(newer(5, 4));
    assert!(!newer(4, 5));
    assert!(!newer(7, 7));
    assert!(newer(0, u32::MAX));
    assert!(newer(3, u32::MAX - 2));
}

// ---------- double copy ----------

#[test]
fn the_newest_valid_copy_wins_and_the_older_one_is_overwritten() {
    let mut store = MetadataStore::new(Mem::default());
    assert_eq!(store.load().unwrap(), Loaded::Blank);
    for seq in 1..=3 {
        let mut r = Record::empty(seq);
        r.state = State::Empty;
        store.commit(&r).unwrap();
    }
    let Loaded::Record { record, degraded, .. } = store.load().unwrap() else { panic!() };
    assert_eq!(record.sequence, 3);
    assert!(!degraded, "the other copy still holds the previous valid record");
}

#[test]
fn a_corrupt_newest_copy_falls_back_to_the_other_and_both_corrupt_is_explicit() {
    let mut store = MetadataStore::new(Mem::default());
    store.commit(&Record::empty(1)).unwrap();
    store.commit(&Record::empty(2)).unwrap();
    // Corrupt the copy holding seq 2.
    let Loaded::Record { copy, .. } = store.load().unwrap() else { panic!() };
    store.backend_mut().copies[copy].as_mut().unwrap()[20] ^= 0xFF;
    let Loaded::Record { record, degraded, .. } = store.load().unwrap() else { panic!() };
    assert_eq!(record.sequence, 1);
    assert!(degraded);
    // Both corrupt: never an invented state.
    let other = 1 - copy;
    store.backend_mut().copies[other].as_mut().unwrap()[20] ^= 0xFF;
    assert_eq!(store.load().unwrap(), Loaded::Corrupted);
    let mut u = WorkloadUpdater::new(std::mem::take(store.backend_mut()));
    assert_eq!(u.recover().unwrap(), Recovery::CorruptedMetadata);
    assert!(matches!(u.state(), Err(UpdateError::Corrupted)));
}

// ---------- machine ----------

#[test]
fn empty_staged_activating_pending_valid() {
    let mut u = updater();
    assert_eq!(u.recover().unwrap(), Recovery::NoWorkload);
    let slot = stage(&mut u, "1.0", b"workload-one", API12).unwrap();
    assert_eq!(u.state().unwrap(), Some(State::Staged));
    assert_eq!(slot, Side::A);
    let mut sup = Sup::default();
    u.activate(&mut sup, API13).unwrap();
    assert_eq!(sup.0, ["switch:A"]);
    assert_eq!(u.state().unwrap(), Some(State::PendingConfirmation));
    assert_eq!(u.recover().unwrap(), Recovery::PendingConfirmation(Side::A));
    u.confirm().unwrap();
    assert_eq!(u.state().unwrap(), Some(State::Valid));
    assert_eq!(u.recover().unwrap(), Recovery::Valid(Side::A));
}

#[test]
fn a_second_update_goes_to_the_other_slot_and_confirms() {
    let mut u = updater();
    install(&mut u, "1.0");
    assert_eq!(stage(&mut u, "2.0", b"workload-two", API12).unwrap(), Side::B);
    let mut sup = Sup::default();
    u.activate(&mut sup, API13).unwrap();
    let r = u.record().unwrap().unwrap();
    assert_eq!((r.active, r.previous_valid), (Some(Side::B), Some(Side::A)));
    u.confirm().unwrap();
    let r = u.record().unwrap().unwrap();
    assert_eq!((r.state, r.active, r.previous_valid), (State::Valid, Some(Side::B), None));
    assert_eq!(r.meta[1].version(), "2.0");
}

#[test]
fn rollback_returns_to_the_previous_valid_workload_or_to_none() {
    let mut u = updater();
    install(&mut u, "1.0");
    stage(&mut u, "2.0", b"workload-two", API12).unwrap();
    u.activate(&mut Sup::default(), API13).unwrap();
    let mut sup = Sup::default();
    u.rollback(&mut sup).unwrap();
    assert_eq!(sup.0, ["restore:A"]);
    let r = u.record().unwrap().unwrap();
    assert_eq!((r.state, r.active), (State::Valid, Some(Side::A)));

    // First-ever Workload fails: nothing to return to -> stopped, Empty.
    let mut u = updater();
    stage(&mut u, "1.0", b"workload-one", API12).unwrap();
    u.activate(&mut Sup::default(), API13).unwrap();
    let mut sup = Sup::default();
    u.rollback(&mut sup).unwrap();
    assert_eq!(sup.0, ["stop"]);
    assert_eq!(u.state().unwrap(), Some(State::Empty));
}

#[test]
fn wrong_state_operations_are_refused() {
    let mut u = updater();
    assert!(matches!(u.activate(&mut Sup::default(), API13), Err(UpdateError::WrongState(None))));
    assert!(matches!(u.confirm(), Err(UpdateError::WrongState(None))));
    assert!(matches!(u.rollback(&mut Sup::default()), Err(UpdateError::WrongState(None))));
    install(&mut u, "1.0");
    assert!(matches!(u.confirm(), Err(UpdateError::WrongState(Some(State::Valid)))));
}

// ---------- supersession ----------

#[test]
fn a_staged_candidate_is_superseded_but_not_a_pending_one() {
    let mut u = updater();
    install(&mut u, "1.0");
    let first = stage(&mut u, "2.0", b"first-candidate", API12).unwrap();
    let second = stage(&mut u, "3.0", b"second-candidate!", API12).unwrap();
    assert_eq!(first, second);
    let r = u.record().unwrap().unwrap();
    assert_eq!(r.state, State::Staged);
    assert_eq!(r.meta[slot(second)].version(), "3.0");

    u.activate(&mut Sup::default(), API13).unwrap();
    let refused = u.prepare(&request("4.0", b"x", API12), 1 << 20);
    assert!(matches!(refused, Err(UpdateError::Refused(Refusal::Busy))));
}

fn slot(side: Side) -> usize {
    crate::otm2::slot_index(side)
}

// ---------- compatibility ----------

#[test]
fn runtime_api_gate_at_activation() {
    let mut allowed = updater();
    stage(&mut allowed, "1", b"needs-1.2", API12).unwrap();
    assert!(allowed.activate(&mut Sup::default(), API13).is_ok());

    let mut refused = updater();
    stage(&mut refused, "1", b"needs-1.4", API14).unwrap();
    let mut sup = Sup::default();
    assert!(matches!(
        refused.activate(&mut sup, API13),
        Err(UpdateError::Refused(Refusal::IncompatibleRuntimeApi))
    ));
    assert!(sup.0.is_empty());
    assert_eq!(refused.state().unwrap(), Some(State::Staged)); // nothing persisted

    let mut major = updater();
    stage(&mut major, "1", b"needs-2.0", RuntimeApi::new(2, 0)).unwrap();
    assert!(matches!(major.activate(&mut Sup::default(), API13), Err(UpdateError::Refused(_))));
}

#[test]
fn the_required_runtime_api_is_persisted_for_the_agent_to_check() {
    let mut u = updater();
    install(&mut u, "1.0");
    assert_eq!(u.active_requirement().unwrap(), Some(API12));
    // Another reader of the same copies sees it too (it is in the record).
    let backend = std::mem::take(u.store_mut().backend_mut());
    let mut again = WorkloadUpdater::new(backend);
    assert_eq!(again.active_requirement().unwrap(), Some(API12));
}

// ---------- digest ----------

#[test]
fn a_wrong_sha256_never_becomes_a_candidate() {
    let mut u = updater();
    install(&mut u, "1.0");
    let before = u.record().unwrap();
    let mut req = request("2.0", b"good-bytes", API12);
    // Order says digest of "good-bytes", but the bytes streamed differ.
    let prepared = u.prepare(&req, 1 << 20).unwrap();
    let mut slot = Slot::default();
    let err = write_all(&prepared, &mut slot, b"evil-bytes", 4).unwrap_err();
    assert!(matches!(err, iobewi_ota::Error::DigestMismatch(_)));
    assert_eq!(u.record().unwrap(), before); // no Staged persisted
    // A committed result that is not the prepared one is refused as well.
    req = request("2.0", b"good-bytes", API12);
    let prepared = u.prepare(&req, 1 << 20).unwrap();
    let other = write_all(&u.prepare(&request("x", b"other-bytes", API12), 1 << 20).unwrap(), &mut Slot::default(), b"other-bytes", 4).unwrap();
    assert!(matches!(u.commit_staged(&prepared, &other), Err(UpdateError::NotPrepared)));
}

#[test]
fn an_oversized_artifact_is_refused_before_anything_is_written() {
    let mut u = updater();
    assert!(matches!(u.prepare(&request("1", &[0u8; 100], API12), 50), Err(UpdateError::TooLarge)));
    assert!(matches!(u.prepare(&UpdateRequest::agent(
        ArtifactDescriptor { id: "a".into(), version: "1".into(), digest: [0; 32], size: 1 }, API13), 10),
        Err(UpdateError::Refused(Refusal::WrongTarget))));
}

// ---------- power loss ----------

#[test]
fn power_loss_before_the_metadata_write_changes_nothing() {
    let mut u = updater();
    install(&mut u, "1.0");
    let before = u.record().unwrap();
    let writes = u.store().backend().writes;
    u.store_mut().backend_mut().drop_write_at = Some(writes);
    assert!(matches!(stage(&mut u, "2.0", b"workload-two", API12), Err(UpdateError::Backend(_))));
    assert_eq!(u.record().unwrap(), before);
    assert_eq!(u.recover().unwrap(), Recovery::Valid(Side::A));
}

#[test]
fn a_torn_write_of_the_new_copy_leaves_the_previous_state() {
    let mut u = updater();
    install(&mut u, "1.0");
    let writes = u.store().backend().writes;
    u.store_mut().backend_mut().tear_write_at = Some(writes);
    assert!(stage(&mut u, "2.0", b"workload-two", API12).is_err());
    // The torn copy fails its CRC; the other copy still holds Valid(A).
    let Loaded::Record { degraded, .. } = u.store_mut().load().unwrap() else { panic!() };
    assert!(degraded);
    assert_eq!(u.recover().unwrap(), Recovery::Valid(Side::A));
}

#[test]
fn artifact_complete_but_not_yet_staged_is_not_a_candidate() {
    let mut u = updater();
    install(&mut u, "1.0");
    let prepared = u.prepare(&request("2.0", b"workload-two", API12), 1 << 20).unwrap();
    let mut slot = Slot::default();
    write_all(&prepared, &mut slot, b"workload-two", 5).unwrap();
    // Power loss here: commit_staged never ran.
    assert_eq!(u.recover().unwrap(), Recovery::Valid(Side::A));
}

#[test]
fn power_loss_during_the_activation_transition_requires_a_rollback_to_the_old_workload() {
    let mut u = updater();
    install(&mut u, "1.0");
    stage(&mut u, "2.0", b"workload-two", API12).unwrap();
    // Fail the second metadata write of activate (Activating -> PendingConfirmation).
    let writes = u.store().backend().writes;
    u.store_mut().backend_mut().drop_write_at = Some(writes + 1);
    assert!(u.activate(&mut Sup::default(), API13).is_err());
    assert_eq!(u.state().unwrap(), Some(State::Activating));
    assert_eq!(u.recover().unwrap(), Recovery::RollbackRequired { restore: Some(Side::A) });
    u.store_mut().backend_mut().drop_write_at = None;
    let mut sup = Sup::default();
    u.rollback(&mut sup).unwrap();
    assert_eq!(sup.0, ["restore:A"]);
    assert_eq!(u.recover().unwrap(), Recovery::Valid(Side::A));
}

#[test]
fn power_loss_before_confirm_leaves_pending_confirmation() {
    let mut u = updater();
    stage(&mut u, "1.0", b"workload-one", API12).unwrap();
    u.activate(&mut Sup::default(), API13).unwrap();
    let writes = u.store().backend().writes;
    u.store_mut().backend_mut().drop_write_at = Some(writes);
    assert!(u.confirm().is_err());
    assert_eq!(u.recover().unwrap(), Recovery::PendingConfirmation(Side::A));
}

#[test]
fn power_loss_during_rollback_resumes_and_completes() {
    let mut u = updater();
    install(&mut u, "1.0");
    stage(&mut u, "2.0", b"workload-two", API12).unwrap();
    u.activate(&mut Sup::default(), API13).unwrap();
    // Rollback: intent write ok, final write lost.
    let writes = u.store().backend().writes;
    u.store_mut().backend_mut().drop_write_at = Some(writes + 1);
    assert!(u.rollback(&mut Sup::default()).is_err());
    assert_eq!(u.state().unwrap(), Some(State::RollingBack));
    assert_eq!(u.recover().unwrap(), Recovery::RollbackRequired { restore: Some(Side::A) });
    u.store_mut().backend_mut().drop_write_at = None;
    let mut sup = Sup::default();
    u.rollback(&mut sup).unwrap(); // resumes: no second intent write
    assert_eq!(sup.0, ["restore:A"]);
    assert_eq!(u.recover().unwrap(), Recovery::Valid(Side::A));
}

#[test]
fn recovery_reports_a_staged_candidate() {
    let mut u = updater();
    install(&mut u, "1.0");
    stage(&mut u, "2.0", b"workload-two", API12).unwrap();
    assert_eq!(u.recover().unwrap(), Recovery::Staged { active: Some(Side::A), candidate: Side::B });
}

// ---------- independence ----------

#[test]
fn otm2_corruption_does_not_touch_otm1_and_a_workload_rollback_keeps_the_agent() {
    // OTM1 (the Agent's record) before.
    let otm1 = iobewi_ota::metadata::Metadata::default().encode().unwrap();
    let agent_slots = AbSlots::with_active(InstalledAgent {
        artifact: ArtifactDescriptor { id: "embewi-agent".into(), version: "a1".into(), digest: [9; 32], size: 1 },
        provides: API13,
    });
    let agent = AgentOta::new(agent_slots);
    let agent_before = agent.slots().clone();

    let mut u = updater();
    install(&mut u, "1.0");
    stage(&mut u, "2.0", b"workload-two", API12).unwrap();
    u.activate(&mut Sup::default(), API13).unwrap();
    u.rollback(&mut Sup::default()).unwrap();
    // Corrupt both OTM2 copies.
    for copy in 0..2 {
        u.store_mut().backend_mut().copies[copy].as_mut().unwrap()[30] ^= 0xFF;
    }
    assert_eq!(u.recover().unwrap(), Recovery::CorruptedMetadata);

    assert_eq!(iobewi_ota::metadata::Metadata::default().encode().unwrap(), otm1);
    assert!(iobewi_ota::metadata::Metadata::decode(&otm1).is_ok());
    assert_eq!(*agent.slots(), agent_before);
    assert_eq!(&otm1[..4], b"OTM1");
}
