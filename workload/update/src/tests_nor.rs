//! Backend tests on a fake NOR flash that behaves like the real thing where it
//! matters: 4 KiB erase units, programming only clears bits, 4-byte write unit,
//! and a crash injected at any erase/program leaves that operation half done.

use std::vec;
use std::vec::Vec;

use iobewi_update_model::{ArtifactDescriptor, RuntimeApi, Side, UpdateRequest, WorkloadSupervisor};
use sha2::{Digest as _, Sha256};

use crate::layout::{
    Kind, LABEL_META, LABEL_SLOT_A, LABEL_SLOT_B, MIN_WORKLOAD_SLOT_SIZE, PartitionSpec, Region, TableError, Unsupported,
    WorkloadLayout, assemble, check_table,
};
use crate::machine::{Recovery, UpdateError, WorkloadActivator, WorkloadUpdater, write_all};
use crate::nor::{NorError, NorMetadata, NorSlot, digest_region, erase_range, read_region};
use crate::otm2::State;
use crate::store::{Loaded, MetadataStore};

const API: RuntimeApi = RuntimeApi::new(1, 3);

use crate::testing::{ERASE, Fake, FakeError};

// ---------- layouts (mirrors of the documented tables) ----------

const fn spec(name: &'static str, kind: Kind, offset: u32, size: u32) -> PartitionSpec {
    PartitionSpec { name, kind, offset, size }
}

const AGENT: [PartitionSpec; 5] = [
    spec("nvs", Kind::Data, 0x9000, 0x6000),
    spec("otadata", Kind::Data, 0xf000, 0x2000),
    spec("phy_init", Kind::Data, 0x11000, 0x1000),
    spec("ota_0", Kind::App, 0x20000, 0x180000),
    spec("ota_1", Kind::App, 0x1a0000, 0x180000),
];

const S3_FLASH: u32 = 0x100_0000;
const S3_WORKLOAD: [PartitionSpec; 3] = [
    spec(LABEL_META, Kind::Data, 0x320000, 0x2000),
    spec(LABEL_SLOT_A, Kind::Data, 0x330000, 0x5E0000),
    spec(LABEL_SLOT_B, Kind::Data, 0x910000, 0x5E0000),
];

const C3_FLASH: u32 = 0x40_0000;
const C3_WORKLOAD: [PartitionSpec; 3] = [
    spec(LABEL_META, Kind::Data, 0x320000, 0x2000),
    spec(LABEL_SLOT_A, Kind::Data, 0x322000, 0x6F000),
    spec(LABEL_SLOT_B, Kind::Data, 0x391000, 0x6F000),
];

fn table(workload: &[PartitionSpec]) -> Vec<PartitionSpec> {
    AGENT.iter().chain(workload.iter()).copied().collect()
}

fn layout_of(workload: &[PartitionSpec]) -> WorkloadLayout {
    assemble(Some(workload[0].region()), Some(workload[1].region()), Some(workload[2].region()), ERASE).unwrap()
}

// ---------- layout / capability ----------

#[test]
fn s3_and_c3_layouts_are_valid_non_overlapping_and_keep_the_agent_partitions() {
    assert_eq!(check_table(&table(&S3_WORKLOAD), S3_FLASH, &AGENT), Ok(()));
    assert_eq!(check_table(&table(&C3_WORKLOAD), C3_FLASH, &AGENT), Ok(()));
    let s3 = layout_of(&S3_WORKLOAD);
    assert_eq!(s3.max_artifact_size(), 0x5E0000);
    let c3 = layout_of(&C3_WORKLOAD);
    assert_eq!(c3.max_artifact_size(), 0x6F000);
    assert!(c3.max_artifact_size() >= MIN_WORKLOAD_SLOT_SIZE);
    // S3 reserve: end of workload_b up to flash end.
    assert_eq!(u64::from(S3_FLASH) - S3_WORKLOAD[2].region().end(), 0x110000);
    // C3 uses the flash exactly.
    assert_eq!(C3_WORKLOAD[2].region().end(), u64::from(C3_FLASH));
}

#[test]
fn the_c3_capacity_scenarios_all_fit_dual_slot() {
    // B: Agent slots 0x170000 ; C: Agent slots 0x150000.
    for (agent_slot, slot_size) in [(0x170000u32, 0x7F000u32), (0x150000, 0x9F000)] {
        let ota_1 = 0x20000 + agent_slot;
        let meta = 0x20000 + 2 * agent_slot;
        let tbl = [
            spec("nvs", Kind::Data, 0x9000, 0x6000),
            spec("otadata", Kind::Data, 0xf000, 0x2000),
            spec("phy_init", Kind::Data, 0x11000, 0x1000),
            spec("ota_0", Kind::App, 0x20000, agent_slot),
            spec("ota_1", Kind::App, ota_1, agent_slot),
            spec(LABEL_META, Kind::Data, meta, 0x2000),
            spec(LABEL_SLOT_A, Kind::Data, meta + 0x2000, slot_size),
            spec(LABEL_SLOT_B, Kind::Data, meta + 0x2000 + slot_size, slot_size),
        ];
        assert_eq!(check_table(&tbl, C3_FLASH, &[]), Ok(()));
        assert_eq!(tbl[7].region().end(), u64::from(C3_FLASH));
        assert!(assemble(Some(tbl[5].region()), Some(tbl[6].region()), Some(tbl[7].region()), ERASE).is_ok());
    }
}

#[test]
fn an_old_layout_or_a_bad_one_is_unsupported_never_guessed() {
    assert_eq!(assemble(None, None, None, ERASE), Err(Unsupported::MissingMeta));
    let m = Some(Region::new(0x320000, 0x2000));
    let a = Some(Region::new(0x330000, 0x5E0000));
    assert_eq!(assemble(m, a, None, ERASE), Err(Unsupported::MissingSlot(Side::B)));
    assert_eq!(assemble(Some(Region::new(0x320000, 0x1000)), a, a, ERASE), Err(Unsupported::MetaTooSmall));
    assert_eq!(assemble(Some(Region::new(0x320800, 0x2000)), a, Some(Region::new(0x910000, 0x5E0000)), ERASE), Err(Unsupported::Misaligned));
    let small = Some(Region::new(0x910000, 0x30000));
    assert_eq!(assemble(m, small, small, ERASE), Err(Unsupported::SlotTooSmall));
    assert_eq!(assemble(m, a, Some(Region::new(0x910000, 0x5D0000)), ERASE), Err(Unsupported::SlotsDiffer));
    assert_eq!(assemble(m, a, a, ERASE), Err(Unsupported::Overlap));
}

#[test]
fn table_checks_catch_overlap_range_alignment_and_agent_changes() {
    let mut t = table(&S3_WORKLOAD);
    t[6] = spec(LABEL_SLOT_A, Kind::Data, 0x1a0000, 0x5E0000); // overlaps ota_1
    assert!(matches!(check_table(&t, S3_FLASH, &AGENT), Err(TableError::Overlap(..))));
    assert!(matches!(check_table(&table(&S3_WORKLOAD), 0x900000, &AGENT), Err(TableError::OutOfFlash(_))));
    let mut t = table(&S3_WORKLOAD);
    t[3] = spec("ota_0", Kind::App, 0x28000, 0x180000); // app not 64 KiB aligned
    assert!(matches!(check_table(&t, S3_FLASH, &AGENT), Err(TableError::Misaligned(_))));
    let mut t = table(&S3_WORKLOAD);
    t[4] = spec("ota_1", Kind::App, 0x1a0000, 0x100000); // Agent partition resized
    assert_eq!(check_table(&t, S3_FLASH, &AGENT), Err(TableError::AgentChanged("ota_1")));
    let mut t = table(&S3_WORKLOAD);
    t[0] = spec("nvs", Kind::Data, 0x8000, 0x6000);
    assert!(matches!(check_table(&t, S3_FLASH, &AGENT), Err(TableError::BelowFirstPartition(_))));
}

// ---------- helpers ----------

fn request(version: &str, data: &[u8]) -> UpdateRequest {
    UpdateRequest::workload(
        ArtifactDescriptor {
            id: "pod".into(),
            version: version.into(),
            digest: Sha256::digest(data).into(),
            size: data.len() as u64,
        },
        API,
    )
}

fn payload(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

#[derive(Default)]
struct Sup;
impl WorkloadSupervisor for Sup {
    fn switch_to(&mut self, _s: Side) {}
    fn restore(&mut self, _s: Side) {}
}
impl WorkloadActivator for Sup {
    fn stop(&mut self) {}
}

type FlashErr = NorError<FakeError>;

/// prepare -> erase the slot range -> verified streaming write -> commit_staged.
fn stage(flash: &mut Fake, layout: WorkloadLayout, version: &str, data: &[u8]) -> Result<Side, UpdateError<FlashErr>> {
    let req = request(version, data);
    let prepared = {
        let mut u = WorkloadUpdater::new(NorMetadata::new(flash, layout));
        u.prepare(&req, u64::from(layout.max_artifact_size()))?
    };
    {
        let mut u = WorkloadUpdater::new(NorMetadata::new(flash, layout));
        u.begin_overwrite()?;
    }
    let region = layout.slot(prepared.slot);
    let units = (data.len() as u64).div_ceil(u64::from(ERASE)) * u64::from(ERASE);
    erase_range(flash, region, 0, units).map_err(UpdateError::Backend)?;
    let mut scratch = vec![0u8; ERASE as usize];
    let committed = {
        let mut slot = NorSlot::new_pre_erased(flash, region, &mut scratch).map_err(UpdateError::Backend)?;
        write_all(&prepared, &mut slot, data, 1000).map_err(|_| UpdateError::NotPrepared)?
    };
    let mut u = WorkloadUpdater::new(NorMetadata::new(flash, layout));
    u.commit_staged(&prepared, &committed)?;
    Ok(prepared.slot)
}

fn with_updater<R>(flash: &mut Fake, layout: WorkloadLayout, f: impl FnOnce(&mut WorkloadUpdater<NorMetadata<'_, Fake>>) -> R) -> R {
    let mut u = WorkloadUpdater::new(NorMetadata::new(flash, layout));
    f(&mut u)
}

fn slot_digest(flash: &mut Fake, layout: WorkloadLayout, side: Side, size: u32) -> [u8; 32] {
    let mut scratch = [0u8; 512];
    digest_region(flash, layout.slot(side), u64::from(size), &mut scratch).unwrap()
}

// ---------- bounds ----------

#[test]
fn every_access_is_bounded_by_its_region() {
    let layout = layout_of(&C3_WORKLOAD);
    let mut flash = Fake::new(C3_FLASH);
    let before = flash.data.clone();
    let region = layout.slot(Side::A);
    let mut scratch = vec![0u8; ERASE as usize];
    // A write that would run past the end of the slot.
    {
        let mut slot = NorSlot::new_pre_erased(&mut flash, region, &mut scratch).unwrap();
        use iobewi_ota::ArtifactStorage;
        let last_unit = u64::from(region.size) - u64::from(ERASE);
        assert!(slot.finish(last_unit, &[1, 2, 3, 4]).is_ok());
        assert_eq!(slot.finish(u64::from(region.size), &[1, 2, 3, 4]), Err(NorError::OutOfBounds));
        assert_eq!(slot.finish(last_unit + u64::from(ERASE), &[9; 4]), Err(NorError::OutOfBounds));
    }
    assert_eq!(erase_range(&mut flash, region, 0, u64::from(region.size) + u64::from(ERASE)), Err(NorError::OutOfBounds));
    assert_eq!(erase_range(&mut flash, region, 0x10, u64::from(ERASE)), Err(NorError::Unaligned));
    let mut buf = [0u8; 8];
    assert_eq!(read_region(&mut flash, region, u64::from(region.size) - 4, &mut buf), Err(NorError::OutOfBounds));
    assert_eq!(read_region(&mut flash, region, u64::MAX - 2, &mut buf), Err(NorError::OutOfBounds));
    // Only the legal 4 bytes in the last unit of slot A changed.
    let diff: Vec<usize> = (0..before.len()).filter(|i| before[*i] != flash.data[*i]).collect();
    let last = (region.offset + region.size - ERASE) as usize;
    assert!(diff.iter().all(|i| (last..last + 4).contains(i)), "unexpected writes: {diff:?}");
    // Metadata copy index out of range.
    use crate::store::MetadataBackend;
    let mut meta = NorMetadata::new(&mut flash, layout);
    assert_eq!(meta.read(2), Err(NorError::OutOfBounds));
}

// ---------- isolation ----------

fn sentinel(flash: &mut Fake, specs: &[PartitionSpec], extra: &[Region]) -> Vec<(Region, Vec<u8>)> {
    let mut snapshots = Vec::new();
    for (n, region) in specs.iter().map(|s| s.region()).chain(extra.iter().copied()).enumerate() {
        for (i, b) in flash.data[region.offset as usize..region.end() as usize].iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(n as u8 * 13) | 1; // never 0xFF-erased-looking
        }
        snapshots.push((region, flash.data[region.offset as usize..region.end() as usize].to_vec()));
    }
    snapshots
}

fn run_lifecycle(flash: &mut Fake, layout: WorkloadLayout) {
    stage(flash, layout, "1.0", &payload(1, 9_000)).unwrap();
    with_updater(flash, layout, |u| {
        u.activate(&mut Sup, API).unwrap();
        u.confirm().unwrap();
    });
    stage(flash, layout, "2.0", &payload(2, 13_000)).unwrap();
    with_updater(flash, layout, |u| {
        u.activate(&mut Sup, API).unwrap();
        u.rollback(&mut Sup).unwrap();
    });
    stage(flash, layout, "3.0", &payload(3, 5_000)).unwrap();
}

#[test]
fn workload_operations_never_touch_the_agent_otm1_or_system_partitions() {
    for (workload, flash_size) in [(&S3_WORKLOAD, S3_FLASH), (&C3_WORKLOAD, C3_FLASH)] {
        let layout = layout_of(workload);
        let mut flash = Fake::new(flash_size);
        // Everything that is not a Workload region gets a sentinel pattern:
        // bootloader + table (0..0x9000), nvs, otadata (OTM1/EWBT), phy_init, ota_0, ota_1.
        let protected_extra = [Region::new(0, 0x9000)];
        let guarded = sentinel(&mut flash, &AGENT, &protected_extra);
        run_lifecycle(&mut flash, layout);
        for (region, bytes) in guarded {
            assert_eq!(&flash.data[region.offset as usize..region.end() as usize], &bytes[..], "region {region:?} changed");
        }
        // Workload regions did change (the test is not vacuous).
        assert!(layout.regions().iter().any(|r| flash.data[r.offset as usize..r.end() as usize].iter().any(|b| *b != 0xFF)));
    }
}

#[test]
fn metadata_and_slot_operations_do_not_touch_each_other() {
    let layout = layout_of(&C3_WORKLOAD);
    let mut flash = Fake::new(C3_FLASH);
    // Slots written first, metadata afterwards.
    let a = payload(5, 20_000);
    let mut scratch = vec![0u8; ERASE as usize];
    let region_a = layout.slot(Side::A);
    erase_range(&mut flash, region_a, 0, 20_480).unwrap();
    {
        let prepared = WorkloadUpdater::new(NorMetadata::new(&mut flash, layout)).prepare(&request("1", &a), 1 << 20).unwrap();
        let mut slot = NorSlot::new_pre_erased(&mut flash, layout.slot(prepared.slot), &mut scratch).unwrap();
        write_all(&prepared, &mut slot, &a, 777).unwrap();
    }
    let slots_before: Vec<Vec<u8>> = [Side::A, Side::B]
        .iter()
        .map(|s| flash.data[layout.slot(*s).offset as usize..layout.slot(*s).end() as usize].to_vec())
        .collect();
    with_updater(&mut flash, layout, |u| u.format().unwrap());
    for (i, s) in [Side::A, Side::B].iter().enumerate() {
        let r = layout.slot(*s);
        assert_eq!(&flash.data[r.offset as usize..r.end() as usize], &slots_before[i][..], "metadata write changed a slot");
    }
    // And slot erase/write leaves both metadata copies intact.
    let meta_before = flash.data[layout.meta().offset as usize..layout.meta().end() as usize].to_vec();
    erase_range(&mut flash, region_a, 0, u64::from(region_a.size)).unwrap();
    assert_eq!(&flash.data[layout.meta().offset as usize..layout.meta().end() as usize], &meta_before[..]);
}

#[test]
fn the_two_metadata_copies_use_separate_erase_units() {
    let layout = layout_of(&S3_WORKLOAD);
    assert_eq!(layout.meta_copy_offset(0), 0);
    assert_eq!(layout.meta_copy_offset(1), ERASE);
    let mut flash = Fake::new(C3_FLASH);
    with_updater(&mut flash, layout_of(&C3_WORKLOAD), |u| u.format().unwrap());
    let layout = layout_of(&C3_WORKLOAD);
    // Commit twice: each copy has been erased once, never both at the same time.
    with_updater(&mut flash, layout, |u| u.format().unwrap());
    let base = layout.meta().offset as usize;
    assert_ne!(&flash.data[base..base + RECORD], &[0xFF; RECORD][..]);
    assert_ne!(&flash.data[base + ERASE as usize..base + ERASE as usize + RECORD], &[0xFF; RECORD][..]);
}

const RECORD: usize = crate::otm2::RECORD_LEN;

// ---------- lifecycle on the fake flash ----------

#[test]
fn full_lifecycle_and_digest_readback() {
    let layout = layout_of(&C3_WORKLOAD);
    let mut flash = Fake::new(C3_FLASH);
    let one = payload(1, 9_000);
    assert_eq!(stage(&mut flash, layout, "1.0", &one).unwrap(), Side::A);
    with_updater(&mut flash, layout, |u| {
        u.activate(&mut Sup, API).unwrap();
        u.confirm().unwrap();
    });
    let two = payload(2, 11_111);
    assert_eq!(stage(&mut flash, layout, "2.0", &two).unwrap(), Side::B);
    assert_eq!(
        with_updater(&mut flash, layout, |u| u.recover().unwrap()),
        Recovery::Staged { active: Some(Side::A), candidate: Side::B }
    );
    // Both artifacts are read back and verified, independently of the metadata.
    assert_eq!(slot_digest(&mut flash, layout, Side::A, one.len() as u32), <[u8; 32]>::from(Sha256::digest(&one)));
    assert_eq!(slot_digest(&mut flash, layout, Side::B, two.len() as u32), <[u8; 32]>::from(Sha256::digest(&two)));
}

#[test]
fn an_artifact_larger_than_the_slot_is_refused_before_any_write() {
    let layout = layout_of(&C3_WORKLOAD);
    let mut flash = Fake::new(C3_FLASH);
    let data = payload(9, layout.max_artifact_size() as usize + 1);
    let before = flash.data.clone();
    assert!(matches!(stage(&mut flash, layout, "big", &data), Err(UpdateError::TooLarge)));
    assert_eq!(flash.data, before);
    // Exactly the slot size is accepted.
    let full = payload(9, layout.max_artifact_size() as usize);
    assert!(stage(&mut flash, layout, "full", &full).is_ok());
}

// ---------- power loss at every flash operation ----------

fn scenario(flash: &mut Fake, layout: WorkloadLayout) {
    let _ = (|| -> Result<(), UpdateError<FlashErr>> {
        stage(flash, layout, "1.0", &payload(1, 9_000))?;
        with_updater(flash, layout, |u| u.activate(&mut Sup, API))?;
        with_updater(flash, layout, |u| u.confirm())?;
        stage(flash, layout, "2.0", &payload(2, 13_000))?;
        with_updater(flash, layout, |u| u.activate(&mut Sup, API))?;
        with_updater(flash, layout, |u| u.rollback(&mut Sup))?;
        stage(flash, layout, "3.0", &payload(3, 5_000))?;
        // Supersede the Staged candidate: its slot is overwritten from the first byte on.
        stage(flash, layout, "4.0", &payload(4, 7_000))?;
        Ok(())
    })();
}

#[test]
fn a_power_cut_at_any_flash_operation_never_loses_or_invents_a_workload() {
    let layout = layout_of(&C3_WORKLOAD);
    let mut counter = Fake::new(C3_FLASH);
    scenario(&mut counter, layout);
    let total = counter.ops;
    assert!(total > 30, "scenario too small: {total}");

    let mut corrupted_cases = 0;
    for crash in 0..total {
        let mut flash = Fake::new(C3_FLASH);
        flash.crash_at = Some(crash);
        scenario(&mut flash, layout);
        flash.reboot();

        let recovery = with_updater(&mut flash, layout, |u| u.recover().unwrap());
        match recovery {
            Recovery::CorruptedMetadata => {
                // Only possible while the very first record is being written.
                corrupted_cases += 1;
                with_updater(&mut flash, layout, |u| u.format().unwrap());
                assert_eq!(with_updater(&mut flash, layout, |u| u.recover().unwrap()), Recovery::NoWorkload);
            }
            Recovery::NoWorkload => {}
            Recovery::Valid(side) | Recovery::PendingConfirmation(side) => {
                // The selected Workload is intact: its digest matches its metadata.
                let record = with_updater(&mut flash, layout, |u| u.record().unwrap().unwrap());
                let meta = record.meta[crate::otm2::slot_index(side)];
                assert_eq!(slot_digest(&mut flash, layout, side, meta.size), meta.digest, "crash {crash}: active slot damaged");
            }
            Recovery::Staged { active, candidate } => {
                let record = with_updater(&mut flash, layout, |u| u.record().unwrap().unwrap());
                let meta = record.meta[crate::otm2::slot_index(candidate)];
                assert_eq!(slot_digest(&mut flash, layout, candidate, meta.size), meta.digest, "crash {crash}: Staged but artifact incomplete");
                if let Some(active) = active {
                    let am = record.meta[crate::otm2::slot_index(active)];
                    assert_eq!(slot_digest(&mut flash, layout, active, am.size), am.digest, "crash {crash}: active slot damaged");
                }
            }
            Recovery::RollbackRequired { restore } => {
                with_updater(&mut flash, layout, |u| u.rollback(&mut Sup).unwrap());
                let after = with_updater(&mut flash, layout, |u| u.recover().unwrap());
                match restore {
                    Some(side) => assert_eq!(after, Recovery::Valid(side)),
                    None => assert_eq!(after, Recovery::NoWorkload),
                }
            }
        }
        // The system keeps working: a fresh update can always be staged afterwards.
        if with_updater(&mut flash, layout, |u| u.recover().unwrap()) != Recovery::NoWorkload {
            let state = with_updater(&mut flash, layout, |u| u.state().unwrap());
            if matches!(state, Some(State::PendingConfirmation)) {
                with_updater(&mut flash, layout, |u| u.confirm().unwrap());
            }
        }
        assert!(stage(&mut flash, layout, "after", &payload(7, 3_000)).is_ok(), "crash {crash}: cannot stage afterwards");
    }
    assert!(corrupted_cases <= 4, "corruption should only affect the first metadata commit, got {corrupted_cases}");
}

#[test]
fn erasing_the_inactive_copy_then_losing_power_keeps_the_previous_copy() {
    let layout = layout_of(&C3_WORKLOAD);
    let mut flash = Fake::new(C3_FLASH);
    stage(&mut flash, layout, "1.0", &payload(1, 9_000)).unwrap();
    with_updater(&mut flash, layout, |u| u.activate(&mut Sup, API).unwrap());
    // The next commit is (erase, program): crash right after the erase.
    flash.crash_at = Some(flash.ops);
    assert!(with_updater(&mut flash, layout, |u| u.confirm()).is_err());
    flash.reboot();
    let mut store = MetadataStore::new(NorMetadata::new(&mut flash, layout));
    let Loaded::Record { record, degraded, .. } = store.load().unwrap() else { panic!() };
    assert_eq!(record.state, State::PendingConfirmation);
    assert!(degraded);
}

#[test]
fn an_erased_wl_meta_makes_the_slots_unselectable_whatever_they_contain() {
    let layout = layout_of(&C3_WORKLOAD);
    let mut flash = Fake::new(C3_FLASH);
    stage(&mut flash, layout, "1.0", &payload(1, 9_000)).unwrap();
    with_updater(&mut flash, layout, |u| u.activate(&mut Sup, API).unwrap());
    with_updater(&mut flash, layout, |u| u.confirm().unwrap());
    // Factory erase: only wl_meta is erased; the slot bytes stay.
    let meta = layout.meta();
    erase_range(&mut flash, Region::new(meta.offset, meta.size), 0, u64::from(meta.size)).unwrap();
    assert_ne!(slot_digest(&mut flash, layout, Side::A, 9_000), [0xFF; 32]);
    assert_eq!(with_updater(&mut flash, layout, |u| u.recover().unwrap()), Recovery::NoWorkload);
    assert!(matches!(with_updater(&mut flash, layout, |u| u.activate(&mut Sup, API)), Err(UpdateError::WrongState(None))));
}
