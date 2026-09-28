//! ESP OTA operations over the process-wide flash owner.
//!
//! The flash mutex is released before the caller accesses ConfigSpace: both
//! backends use the same non-reentrant physical flash capability.

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use iobewi_esp_flash::{EspFlash, SharedFlash};
use fibewi::{BackendOutcome, Committed, Digest, Error, WriteSession};
use sha2::{Digest as _, Sha256};

use crate::{
    AppPartition, AppSlot, EspArtifactStorage, FlashWriteError,
    PARTITION_TABLE_BUFFER_SIZE, erase_partition_range, otadata,
};

fn table_buffer() -> Box<otadata::TableBuffer> {
    Box::new([0u8; PARTITION_TABLE_BUFFER_SIZE])
}

pub fn write_target_locked(flash: &mut EspFlash) -> Result<AppPartition, otadata::Error> {
    otadata::write_target(flash.storage(), &mut table_buffer())
}

pub async fn write_target(flash: &SharedFlash) -> Result<AppPartition, otadata::Error> {
    let mut guard = flash.lock().await;
    write_target_locked(&mut guard)
}

pub async fn confirm(flash: &SharedFlash) -> Result<(), otadata::Error> {
    let mut guard = flash.lock().await;
    otadata::confirm(guard.storage(), &mut table_buffer())
}

pub async fn reject(flash: &SharedFlash) -> Result<(), otadata::Error> {
    let mut guard = flash.lock().await;
    otadata::reject(guard.storage(), &mut table_buffer())
}

pub async fn activate(flash: &SharedFlash, target: AppSlot) -> Result<(), otadata::Error> {
    let mut guard = flash.lock().await;
    otadata::activate(guard.storage(), &mut table_buffer(), target)
}

/// The partition actually booted, including after an image-header fallback.
pub async fn active_slot(flash: &SharedFlash) -> &'static str {
    let mut guard = flash.lock().await;
    otadata::booted_slot(guard.storage(), &mut table_buffer()).unwrap_or("")
}

pub async fn image_outcome(flash: &SharedFlash) -> BackendOutcome {
    let mut guard = flash.lock().await;
    otadata::read_entries(guard.storage(), &mut table_buffer())
        .map(|entries| otadata::image_outcome(&entries))
        .unwrap_or(BackendOutcome::Other)
}

/// Raw bootloader state for the agent's status endpoint.
pub async fn boot_info(flash: &SharedFlash) -> otadata::BootEntry {
    const UNKNOWN: otadata::BootEntry = otadata::BootEntry {
        slot: "", seq: 0, state: "unknown",
    };
    let mut guard = flash.lock().await;
    otadata::read_entries(guard.storage(), &mut table_buffer())
        .ok()
        .and_then(|entries| otadata::boot_entry(&entries))
        .unwrap_or(UNKNOWN)
}

pub const fn erase_batch_size() -> u64 { 64 * 1024 }

pub const fn erase_size() -> usize { <EspFlash as NorFlash>::ERASE_SIZE }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreloadedError { NoTarget, TooLarge, Flash }

/// Hash the factory-preloaded image in the inactive slot before FiBeWI
/// publishes a staged transaction. The flash lock is released before any
/// ConfigSpace commit by the caller.
pub async fn hash_preloaded(flash: &SharedFlash, size: u32) -> Result<(AppSlot, Digest), PreloadedError> {
    let mut guard = flash.lock().await;
    let target = write_target_locked(&mut guard).map_err(|_| PreloadedError::NoTarget)?;
    if size as usize > target.size {
        return Err(PreloadedError::TooLarge);
    }
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 4096];
    let mut offset = 0u32;
    while offset < size {
        let take = ((size - offset) as usize).min(buf.len());
        ReadNorFlash::read(guard.storage(), target.offset + offset, &mut buf[..take])
            .map_err(|_| PreloadedError::Flash)?;
        hasher.update(&buf[..take]);
        offset += take as u32;
    }
    Ok((target.slot, Digest(hasher.finalize().into())))
}

/// ESP physical state for one FiBeWI upload session. The FiBeWI engine keeps
/// the received/durable watermarks and digest; only flash geometry lives here.
pub struct ArtifactWriter {
    partition: AppPartition,
    scratch: Box<[u8]>,
    erased_through: u64,
    sectors_flushed: u32,
    erase_batches: u32,
}

impl ArtifactWriter {
    pub fn new(partition: AppPartition) -> Self {
        Self {
            partition,
            scratch: vec![0u8; erase_size()].into_boxed_slice(),
            erased_through: 0,
            sectors_flushed: 0,
            erase_batches: 0,
        }
    }

    pub fn slot(&self) -> AppSlot { self.partition.slot }
    pub fn sectors_flushed(&self) -> u32 { self.sectors_flushed }
    pub fn erase_batches(&self) -> u32 { self.erase_batches }

    /// Erase ahead in 64 KiB blocks while FiBeWI's durable watermark still
    /// advances only after each sector has been programmed successfully.
    pub async fn append(&mut self, flash: &SharedFlash, engine: &mut WriteSession, data: &[u8]) -> bool {
        if !engine.can_append(data.len()) {
            return false;
        }
        let end_received = engine.received() + data.len() as u64;
        let erase_batch = erase_batch_size();
        let desired_erased = end_received
            .div_ceil(erase_batch)
            .saturating_mul(erase_batch)
            .min(self.partition.size as u64);

        let mut flash_guard = flash.lock().await;
        let before = engine.durable();
        let raw_flash = flash_guard.storage();
        let ok = if desired_erased > self.erased_through {
            if erase_partition_range(raw_flash, self.partition, self.erased_through, desired_erased).is_err() {
                false
            } else {
                self.erase_batches += ((desired_erased - self.erased_through) / erase_batch) as u32;
                self.erased_through = desired_erased;
                match EspArtifactStorage::new_pre_erased(raw_flash, self.partition, self.scratch.as_mut()) {
                    Ok(mut backend) => engine.append(&mut backend, data).is_ok(),
                    Err(_) => false,
                }
            }
        } else {
            match EspArtifactStorage::new_pre_erased(raw_flash, self.partition, self.scratch.as_mut()) {
                Ok(mut backend) => engine.append(&mut backend, data).is_ok(),
                Err(_) => false,
            }
        };
        self.sectors_flushed += (engine.durable() - before).div_ceil(erase_size() as u64) as u32;
        ok
    }

    /// Flush a final partial sector after every received byte was covered
    /// by a prior erase. Returns FiBeWI's digest/completeness decision.
    pub async fn finish(
        &mut self,
        flash: &SharedFlash,
        engine: WriteSession,
    ) -> Result<Committed, Error<FlashWriteError>> {
        if engine.received() > self.erased_through {
            return Err(Error::Incomplete { durable: engine.durable() });
        }
        let before = engine.durable();
        let mut guard = flash.lock().await;
        let mut backend = EspArtifactStorage::new_pre_erased(
            guard.storage(), self.partition, self.scratch.as_mut(),
        ).map_err(Error::Backend)?;
        let committed = engine.finish(&mut backend)?;
        self.sectors_flushed += (committed.size - before).div_ceil(erase_size() as u64) as u32;
        Ok(committed)
    }
}
