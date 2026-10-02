//! One-lock-per-operation Workload storage, generic over the platform's flash
//! lock. The platform implements [`FlashAccess`] (on ESP: take the single
//! `SharedFlash` mutex, run the closure, release it); everything else -- OTM2
//! metadata, slot writes, read-back digests -- is portable and host-tested on a
//! fake NOR flash.
//!
//! The contract of [`FlashAccess::with`]: the closure runs while the lock is
//! held and **must not await** (it cannot -- it is synchronous), so a caller
//! never holds the flash across HTTP parsing, TLS or another storage layer.

use alloc::boxed::Box;
use alloc::vec;
use embedded_storage::nor_flash::{ErrorType, NorFlash};
use iobewi_ota::{Committed, Error as EngineError, WriteSession};
use iobewi_update_model::{RuntimeApi, Side, UpdateRequest, WorkloadSupervisor};
use sha2::{Digest as _, Sha256};

use crate::layout::{Region, WorkloadLayout};
use crate::machine::{Prepared, Recovery, UpdateError, WorkloadActivator, WorkloadUpdater};
use crate::nor::{NorError, NorMetadata, NorSlot, erase_range, read_region};
use crate::otm2::Record;

/// Exclusive access to the physical flash for the duration of one closure.
#[allow(async_fn_in_trait)]
pub trait FlashAccess {
    type Flash: NorFlash;

    async fn with<R>(&self, f: impl FnOnce(&mut Self::Flash) -> R) -> R;
}

pub type StorageError<A> = NorError<<<A as FlashAccess>::Flash as ErrorType>::Error>;

/// Erase-ahead block size: 64 KiB, the NOR block-erase unit (one lock per block).
pub const ERASE_BATCH: u64 = 64 * 1024;

/// Workload storage on one device. Construct it only from a validated layout.
pub struct WorkloadFlash<A> {
    layout: WorkloadLayout,
    access: A,
}

impl<A: FlashAccess> WorkloadFlash<A> {
    pub fn new(layout: WorkloadLayout, access: A) -> Self {
        Self { layout, access }
    }

    pub fn layout(&self) -> &WorkloadLayout {
        &self.layout
    }

    pub fn access(&self) -> &A {
        &self.access
    }

    /// The updater is stateless (the state is in flash), so it is built per
    /// operation around the locked flash.
    async fn with_updater<R>(
        &self,
        f: impl FnOnce(&mut WorkloadUpdater<NorMetadata<'_, A::Flash>>) -> R,
    ) -> R {
        let layout = self.layout;
        self.access
            .with(|flash| {
                let mut updater = WorkloadUpdater::new(NorMetadata::new(flash, layout));
                f(&mut updater)
            })
            .await
    }

    pub async fn recover(&self) -> Result<Recovery, StorageError<A>> {
        self.with_updater(|u| u.recover()).await
    }

    /// Explicit recovery from corrupted metadata (or an OTM2 factory reset).
    pub async fn format(&self) -> Result<(), StorageError<A>> {
        self.with_updater(|u| u.format()).await
    }

    pub async fn record(&self) -> Result<Option<Record>, UpdateError<StorageError<A>>> {
        self.with_updater(|u| u.record()).await
    }

    /// Reserves the inactive slot; refuses an artifact larger than a slot
    /// before any byte is written.
    pub async fn prepare(&self, request: &UpdateRequest) -> Result<Prepared, UpdateError<StorageError<A>>> {
        let capacity = u64::from(self.layout.max_artifact_size());
        self.with_updater(|u| u.prepare(request, capacity)).await
    }

    pub async fn commit_staged(&self, prepared: &Prepared, committed: &Committed) -> Result<(), UpdateError<StorageError<A>>> {
        self.with_updater(|u| u.commit_staged(prepared, committed)).await
    }

    /// Side-effect-free activation check (state `Staged`, runtime API met).
    pub async fn preflight_activate(&self, agent_api: RuntimeApi) -> Result<Record, UpdateError<StorageError<A>>> {
        self.with_updater(|u| u.preflight_activate(agent_api)).await
    }

    /// The supervisor is called with the flash lock held; it is synchronous, so
    /// it cannot wait for flash and cannot deadlock on it.
    pub async fn activate<S: WorkloadSupervisor>(&self, supervisor: &mut S, agent_api: RuntimeApi) -> Result<(), UpdateError<StorageError<A>>> {
        self.with_updater(|u| u.activate(supervisor, agent_api)).await
    }

    pub async fn confirm(&self) -> Result<(), UpdateError<StorageError<A>>> {
        self.with_updater(|u| u.confirm()).await
    }

    pub async fn rollback<S: WorkloadActivator>(&self, activator: &mut S) -> Result<(), UpdateError<StorageError<A>>> {
        self.with_updater(|u| u.rollback(activator)).await
    }

    /// Streaming writer for a prepared slot (erase-ahead in 64 KiB blocks, one
    /// erase unit durable at a time).
    pub fn writer(&self, prepared: &Prepared) -> SlotWriter {
        SlotWriter {
            region: self.layout.slot(prepared.slot),
            session: Some(prepared.session()),
            scratch: vec![0u8; <A::Flash as NorFlash>::ERASE_SIZE].into_boxed_slice(),
            erased_through: 0,
            erase_batches: 0,
        }
    }

    /// SHA-256 of the first `size` bytes of a slot, read back from flash one
    /// erase unit per lock so NVS and the Agent OTA metadata are never starved.
    pub async fn read_digest(&self, side: Side, size: u32) -> Result<[u8; 32], StorageError<A>> {
        let region = self.layout.slot(side);
        let mut hasher = Sha256::new();
        let mut chunk = vec![0u8; <A::Flash as NorFlash>::ERASE_SIZE];
        let mut offset = 0u64;
        while offset < u64::from(size) {
            let take = ((u64::from(size) - offset) as usize).min(chunk.len());
            self.access
                .with(|flash| read_region(flash, region, offset, &mut chunk[..take]))
                .await?;
            hasher.update(&chunk[..take]);
            offset += take as u64;
        }
        Ok(hasher.finalize().into())
    }

    /// Erases a whole slot (maintenance / tests); normally the writer erases ahead.
    pub async fn erase_slot(&self, side: Side) -> Result<(), StorageError<A>> {
        let region = self.layout.slot(side);
        self.access.with(|flash| erase_range(flash, region, 0, u64::from(region.size))).await
    }
}

/// Physical state of one Workload slot upload; the common engine
/// (`WriteSession`) keeps the received/durable watermarks and the digest.
pub struct SlotWriter {
    region: Region,
    session: Option<WriteSession>,
    scratch: Box<[u8]>,
    erased_through: u64,
    erase_batches: u32,
}

impl SlotWriter {
    pub fn received(&self) -> u64 {
        self.session.as_ref().map_or(0, WriteSession::received)
    }

    pub fn durable(&self) -> u64 {
        self.session.as_ref().map_or(0, WriteSession::durable)
    }

    pub fn erase_batches(&self) -> u32 {
        self.erase_batches
    }

    /// Appends `data`, erasing ahead in 64 KiB blocks. `false` if the chunk does
    /// not fit the declared size or the flash refused.
    pub async fn append<A: FlashAccess>(&mut self, access: &A, data: &[u8]) -> bool {
        let region = self.region;
        let erased_through = self.erased_through;
        let Some(session) = self.session.as_mut() else { return false };
        if !session.can_append(data.len()) {
            return false;
        }
        let end = session.received() + data.len() as u64;
        let wanted = end.div_ceil(ERASE_BATCH).saturating_mul(ERASE_BATCH).min(u64::from(region.size));
        let scratch = &mut self.scratch;
        let ok = access
            .with(|flash| {
                if wanted > erased_through && erase_range(flash, region, erased_through, wanted).is_err() {
                    return None;
                }
                let Ok(mut backend) = NorSlot::new_pre_erased(flash, region, scratch) else { return None };
                session.append(&mut backend, data).is_ok().then_some(wanted.max(erased_through))
            })
            .await;
        match ok {
            Some(erased) => {
                if erased > self.erased_through {
                    self.erase_batches += ((erased - self.erased_through).div_ceil(ERASE_BATCH)) as u32;
                    self.erased_through = erased;
                }
                true
            }
            None => false,
        }
    }

    /// Flushes the last partial unit and returns the engine's verdict on size
    /// and SHA-256.
    pub async fn finish<A: FlashAccess>(&mut self, access: &A) -> Result<Committed, EngineError<StorageError<A>>> {
        let session = self.session.take().ok_or(EngineError::NotStaged)?;
        if session.received() > self.erased_through {
            return Err(EngineError::Incomplete { durable: session.durable() });
        }
        let region = self.region;
        let scratch = &mut self.scratch;
        access
            .with(|flash| {
                let mut backend = NorSlot::new_pre_erased(flash, region, scratch).map_err(EngineError::Backend)?;
                session.finish(&mut backend)
            })
            .await
    }
}
