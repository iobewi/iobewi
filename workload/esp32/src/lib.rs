#![no_std]

//! ESP storage backend of the Workload OTA (S16).
//!
//! * **Where**: the three partitions `wl_meta`, `workload_a`, `workload_b` are
//!   found **by name** in the on-flash partition table (data type, subtype
//!   `undefined`). No offset or size lives in Rust: the table is the single
//!   source of truth. A table without them -- an older device, a C3 layout
//!   without Workload space -- is [`Capability::Unsupported`], never a guess at
//!   free flash.
//! * **How**: every operation locks the one process-wide
//!   [`SharedFlash`](iobewi_esp_flash::SharedFlash) for exactly one step and
//!   releases it before returning (the same single-mutex contract as the Agent
//!   OTA adapter); no second mutex, no second `FlashStorage`. The portable
//!   engine (`iobewi-workload-ota`) and the generic NOR backend do the work;
//!   this crate only adds table lookup and the lock.
//! * **Not here**: loading, running or health-checking a Workload; HTTP; the
//!   Agent's OTA (`ota_0`/`ota_1`/`otadata` are never touched: the backend only
//!   ever receives the three validated Workload regions).

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use embedded_storage::nor_flash::{ErrorType, NorFlash};
use esp_bootloader_esp_idf::partitions::{PARTITION_TABLE_MAX_LEN, read_partition_table};
use iobewi_esp_flash::{EspFlash, SharedFlash};
use iobewi_ota::{Committed, Digest, Error as EngineError, WriteSession};
use iobewi_update_model::{RuntimeApi, Side, UpdateRequest, WorkloadSupervisor};
use iobewi_workload_ota::layout::{
    LABEL_META, LABEL_SLOT_A, LABEL_SLOT_B, Region, Unsupported, WorkloadLayout, assemble,
};
use iobewi_workload_ota::machine::{Prepared, Recovery, UpdateError, WorkloadActivator, WorkloadUpdater};
use iobewi_workload_ota::nor::{NorError, NorMetadata, NorSlot, digest_region, erase_range};
use sha2::{Digest as _, Sha256};

pub use iobewi_workload_ota::layout::{MIN_WORKLOAD_SLOT_SIZE, WorkloadLayout as Layout};

/// ESP-IDF partition type "data" and subtype "undefined": the only data
/// subtype the Agent's table parser accepts besides the named ones, so the
/// Workload partitions are told apart by label.
const TYPE_DATA: u8 = 0x01;
const SUBTYPE_UNDEFINED: u8 = 0x06;

pub type FlashError = <EspFlash as ErrorType>::Error;
pub type StorageError = NorError<FlashError>;

/// Does this device have Workload storage?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Supported(WorkloadLayout),
    /// The table has no (valid) Workload layout: Workload OTA is unavailable.
    Unsupported(Unsupported),
    /// The partition table could not be read.
    TableUnreadable,
}

impl Capability {
    pub fn is_supported(&self) -> bool {
        matches!(self, Capability::Supported(_))
    }
}

/// Reads the table and assembles the Workload layout. A Workload partition that
/// overlaps any other partition makes the whole device `Unsupported`.
pub async fn probe(flash: &SharedFlash) -> Capability {
    let mut buffer = Box::new([0u8; PARTITION_TABLE_MAX_LEN]);
    let mut guard = flash.lock().await;
    let Ok(table) = read_partition_table(guard.storage(), &mut buffer[..]) else {
        return Capability::TableUnreadable;
    };
    let (mut meta, mut a, mut b) = (None, None, None);
    for entry in table.iter() {
        if entry.raw_type() != TYPE_DATA || entry.raw_subtype() != SUBTYPE_UNDEFINED {
            continue;
        }
        let region = Region::new(entry.offset(), entry.len());
        match entry.label_as_str() {
            LABEL_META => meta = Some(region),
            LABEL_SLOT_A => a = Some(region),
            LABEL_SLOT_B => b = Some(region),
            _ => {}
        }
    }
    let layout = match assemble(meta, a, b, <EspFlash as NorFlash>::ERASE_SIZE as u32) {
        Ok(layout) => layout,
        Err(why) => return Capability::Unsupported(why),
    };
    // None of the Workload regions may overlap another partition (Agent slots,
    // otadata, nvs...). Entries carrying a Workload label are the regions themselves.
    for entry in table.iter() {
        let other = Region::new(entry.offset(), entry.len());
        let is_workload = layout.regions().contains(&other);
        if !is_workload && layout.regions().iter().any(|r| r.overlaps(&other)) {
            return Capability::Unsupported(Unsupported::Overlap);
        }
    }
    Capability::Supported(layout)
}

/// Workload storage on one device. Construct it only from a supported layout.
#[derive(Debug, Clone, Copy)]
pub struct EspWorkloadStorage {
    layout: WorkloadLayout,
}

impl EspWorkloadStorage {
    pub fn new(layout: WorkloadLayout) -> Self {
        Self { layout }
    }

    pub fn layout(&self) -> &WorkloadLayout {
        &self.layout
    }

    /// Locks the flash for one metadata operation (the updater itself is
    /// stateless: the state is in flash).
    async fn with_updater<R>(
        &self,
        flash: &SharedFlash,
        f: impl FnOnce(&mut WorkloadUpdater<NorMetadata<'_, EspFlash>>) -> R,
    ) -> R {
        let mut guard = flash.lock().await;
        let mut updater = WorkloadUpdater::new(NorMetadata::new(&mut *guard, self.layout));
        f(&mut updater)
    }

    pub async fn recover(&self, flash: &SharedFlash) -> Result<Recovery, StorageError> {
        self.with_updater(flash, |u| u.recover()).await
    }

    /// Explicit recovery from corrupted metadata (or an OTM2 factory reset).
    pub async fn format(&self, flash: &SharedFlash) -> Result<(), StorageError> {
        self.with_updater(flash, |u| u.format()).await
    }

    /// Reserves the inactive slot; refuses an artifact larger than a slot
    /// before any byte is written.
    pub async fn prepare(&self, flash: &SharedFlash, request: &UpdateRequest) -> Result<Prepared, UpdateError<StorageError>> {
        let capacity = u64::from(self.layout.max_artifact_size());
        self.with_updater(flash, |u| u.prepare(request, capacity)).await
    }

    /// Persists `Staged` after a verified write.
    pub async fn commit_staged(&self, flash: &SharedFlash, prepared: &Prepared, committed: &Committed) -> Result<(), UpdateError<StorageError>> {
        self.with_updater(flash, |u| u.commit_staged(prepared, committed)).await
    }

    /// The supervisor is called with the flash lock held: it must not wait for
    /// flash access (it cannot -- the call is synchronous).
    pub async fn activate<S: WorkloadSupervisor>(&self, flash: &SharedFlash, supervisor: &mut S, agent_api: RuntimeApi) -> Result<(), UpdateError<StorageError>> {
        self.with_updater(flash, |u| u.activate(supervisor, agent_api)).await
    }

    pub async fn confirm(&self, flash: &SharedFlash) -> Result<(), UpdateError<StorageError>> {
        self.with_updater(flash, |u| u.confirm()).await
    }

    pub async fn rollback<A: WorkloadActivator>(&self, flash: &SharedFlash, activator: &mut A) -> Result<(), UpdateError<StorageError>> {
        self.with_updater(flash, |u| u.rollback(activator)).await
    }

    pub async fn record(&self, flash: &SharedFlash) -> Result<Option<iobewi_workload_ota::otm2::Record>, UpdateError<StorageError>> {
        self.with_updater(flash, |u| u.record()).await
    }

    /// Streaming writer for a prepared slot (erase-ahead in 64 KiB blocks, one
    /// erase unit durable at a time).
    pub fn writer(&self, prepared: &Prepared) -> SlotWriter {
        SlotWriter {
            region: self.layout.slot(prepared.slot),
            session: Some(prepared.session()),
            scratch: vec![0u8; <EspFlash as NorFlash>::ERASE_SIZE].into_boxed_slice(),
            erased_through: 0,
            erase_units: 0,
        }
    }

    /// SHA-256 of the first `size` bytes of a slot, read back from flash one
    /// erase unit per lock so NVS and the Agent OTA metadata are never starved.
    pub async fn read_digest(&self, flash: &SharedFlash, side: Side, size: u32) -> Result<[u8; 32], StorageError> {
        let region = self.layout.slot(side);
        let mut hasher = Sha256::new();
        let mut chunk = vec![0u8; <EspFlash as NorFlash>::ERASE_SIZE];
        let mut offset = 0u64;
        while offset < u64::from(size) {
            let take = ((u64::from(size) - offset) as usize).min(chunk.len());
            {
                let mut guard = flash.lock().await;
                iobewi_workload_ota::nor::read_region(&mut *guard, region, offset, &mut chunk[..take])?;
            }
            hasher.update(&chunk[..take]);
            offset += take as u64;
        }
        Ok(hasher.finalize().into())
    }

    /// Same, over the already-computed digest type of the engine.
    pub async fn verify(&self, flash: &SharedFlash, side: Side, size: u32, expected: [u8; 32]) -> Result<bool, StorageError> {
        Ok(self.read_digest(flash, side, size).await? == expected)
    }

    /// Erases a whole slot (maintenance / tests); normally the writer erases ahead.
    pub async fn erase_slot(&self, flash: &SharedFlash, side: Side) -> Result<(), StorageError> {
        let region = self.layout.slot(side);
        let mut guard = flash.lock().await;
        erase_range(&mut *guard, region, 0, u64::from(region.size))
    }

    /// One-shot digest of a slot while holding the lock the whole time (only for
    /// small artifacts / host-like use).
    pub async fn read_digest_locked(&self, flash: &SharedFlash, side: Side, size: u32) -> Result<Digest, StorageError> {
        let mut guard = flash.lock().await;
        let mut scratch = [0u8; 512];
        digest_region(&mut *guard, self.layout.slot(side), u64::from(size), &mut scratch).map(Digest)
    }
}

/// Erase-ahead block size: 64 KiB, the NOR block-erase unit (one lock per block).
pub const fn erase_batch_size() -> u64 {
    64 * 1024
}

/// Physical state of one Workload slot upload; the common engine
/// (`WriteSession`) keeps the received/durable watermarks and the digest.
pub struct SlotWriter {
    region: Region,
    session: Option<WriteSession>,
    scratch: alloc::boxed::Box<[u8]>,
    erased_through: u64,
    erase_units: u32,
}

impl SlotWriter {
    pub fn received(&self) -> u64 {
        self.session.as_ref().map_or(0, WriteSession::received)
    }

    pub fn durable(&self) -> u64 {
        self.session.as_ref().map_or(0, WriteSession::durable)
    }

    pub fn erase_batches(&self) -> u32 {
        self.erase_units
    }

    /// Appends `data`, erasing ahead in 64 KiB blocks. `false` if the chunk does
    /// not fit the declared size or the flash refused.
    pub async fn append(&mut self, flash: &SharedFlash, data: &[u8]) -> bool {
        let Some(session) = self.session.as_mut() else { return false };
        if !session.can_append(data.len()) {
            return false;
        }
        let end = session.received() + data.len() as u64;
        let batch = erase_batch_size();
        let wanted = end.div_ceil(batch).saturating_mul(batch).min(u64::from(self.region.size));
        let mut guard = flash.lock().await;
        if wanted > self.erased_through {
            if erase_range(&mut *guard, self.region, self.erased_through, wanted).is_err() {
                return false;
            }
            self.erase_units += ((wanted - self.erased_through) / batch) as u32 + u32::from((wanted - self.erased_through) % batch != 0);
            self.erased_through = wanted;
        }
        let Ok(mut backend) = NorSlot::new_pre_erased(&mut *guard, self.region, &mut self.scratch) else {
            return false;
        };
        session.append(&mut backend, data).is_ok()
    }

    /// Flushes the last partial unit and returns the engine's verdict on size
    /// and SHA-256.
    pub async fn finish(&mut self, flash: &SharedFlash) -> Result<Committed, EngineError<StorageError>> {
        let session = self.session.take().ok_or(EngineError::NotStaged)?;
        if session.received() > self.erased_through {
            return Err(EngineError::Incomplete { durable: session.durable() });
        }
        let mut guard = flash.lock().await;
        let mut backend = NorSlot::new_pre_erased(&mut *guard, self.region, &mut self.scratch).map_err(EngineError::Backend)?;
        session.finish(&mut backend)
    }
}
