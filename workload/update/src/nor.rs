//! Generic NOR-flash backend (`embedded-storage`) for the Workload regions:
//! the two OTM2 metadata copies and the two slots. It knows regions (from
//! [`crate::layout`]), never a platform: every access is bounds-checked against
//! the target region, so no operation can reach another partition.
//!
//! Metadata copy `i` lives in its own erase unit (`meta.offset + i * erase`), so
//! erasing one copy can never destroy the other. A metadata write is
//! erase-then-program; an interruption leaves that copy blank or torn (CRC
//! fails) and the other copy still holds the last good record.

use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use iobewi_ota::ArtifactStorage;
use sha2::{Digest as _, Sha256};

use crate::layout::{Region, WorkloadLayout};
use crate::otm2::RECORD_LEN;
use crate::store::{COPIES, MetadataBackend};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NorError<E> {
    Flash(E),
    /// offset/length (or their sum) leaves the target region.
    OutOfBounds,
    /// Not aligned to the erase unit where that is required.
    Unaligned,
    /// The caller's scratch buffer is smaller than one erase unit.
    ScratchTooSmall,
}

/// Absolute offset of `[offset, offset + len)` inside `region`, or `OutOfBounds`.
fn locate<E>(region: Region, offset: u64, len: u64) -> Result<u32, NorError<E>> {
    let end = offset.checked_add(len).ok_or(NorError::OutOfBounds)?;
    if end > u64::from(region.size) {
        return Err(NorError::OutOfBounds);
    }
    let absolute = u64::from(region.offset) + offset;
    u32::try_from(absolute).map_err(|_| NorError::OutOfBounds)
}

/// Erases `[from, to)` (relative to `region`, erase-aligned).
pub fn erase_range<F: NorFlash>(
    flash: &mut F,
    region: Region,
    from: u64,
    to: u64,
) -> Result<(), NorError<F::Error>> {
    if to < from {
        return Err(NorError::OutOfBounds);
    }
    let erase = F::ERASE_SIZE as u64;
    if from % erase != 0 || to % erase != 0 {
        return Err(NorError::Unaligned);
    }
    if from == to {
        return Ok(());
    }
    let start = locate::<F::Error>(region, from, to - from)?;
    flash.erase(start, start + (to - from) as u32).map_err(NorError::Flash)
}

/// Reads `buf.len()` bytes at `offset` (relative to `region`).
pub fn read_region<F: ReadNorFlash>(
    flash: &mut F,
    region: Region,
    offset: u64,
    buf: &mut [u8],
) -> Result<(), NorError<F::Error>> {
    let start = locate::<F::Error>(region, offset, buf.len() as u64)?;
    flash.read(start, buf).map_err(NorError::Flash)
}

/// SHA-256 of the first `size` bytes of a slot, read back from flash (the
/// authoritative check after a write or a reboot). `scratch` sizes the reads.
pub fn digest_region<F: ReadNorFlash>(
    flash: &mut F,
    region: Region,
    size: u64,
    scratch: &mut [u8],
) -> Result<[u8; 32], NorError<F::Error>> {
    if scratch.is_empty() {
        return Err(NorError::ScratchTooSmall);
    }
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    while offset < size {
        let take = ((size - offset) as usize).min(scratch.len());
        read_region(flash, region, offset, &mut scratch[..take])?;
        hasher.update(&scratch[..take]);
        offset += take as u64;
    }
    Ok(hasher.finalize().into())
}

/// The two OTM2 metadata copies over `wl_meta`.
pub struct NorMetadata<'a, F> {
    flash: &'a mut F,
    layout: WorkloadLayout,
}

impl<'a, F: NorFlash> NorMetadata<'a, F> {
    pub fn new(flash: &'a mut F, layout: WorkloadLayout) -> Self {
        Self { flash, layout }
    }

    fn copy_region(&self, copy: usize) -> Result<Region, NorError<F::Error>> {
        if copy >= COPIES {
            return Err(NorError::OutOfBounds);
        }
        Ok(Region::new(self.layout.meta().offset + self.layout.meta_copy_offset(copy), self.layout.erase_size()))
    }
}

impl<F: NorFlash> MetadataBackend for NorMetadata<'_, F> {
    type Error = NorError<F::Error>;

    fn read(&mut self, copy: usize) -> Result<[u8; RECORD_LEN], Self::Error> {
        let region = self.copy_region(copy)?;
        let mut out = [0u8; RECORD_LEN];
        read_region(self.flash, region, 0, &mut out)?;
        Ok(out)
    }

    fn write(&mut self, copy: usize, bytes: &[u8; RECORD_LEN]) -> Result<(), Self::Error> {
        let region = self.copy_region(copy)?;
        // Pad to the programming unit with the erased value.
        const MAX_PAD: usize = RECORD_LEN + 16;
        let padded = RECORD_LEN.div_ceil(F::WRITE_SIZE) * F::WRITE_SIZE;
        if padded > MAX_PAD {
            return Err(NorError::ScratchTooSmall);
        }
        let mut buf = [0xFFu8; MAX_PAD];
        buf[..RECORD_LEN].copy_from_slice(bytes);
        erase_range(self.flash, region, 0, u64::from(region.size))?;
        let start = locate::<F::Error>(region, 0, padded as u64)?;
        self.flash.write(start, &buf[..padded]).map_err(NorError::Flash)
    }
}

/// Sector-buffered [`ArtifactStorage`] over one Workload slot: bytes are made
/// durable one erase unit at a time (the engine's watermark moves only after a
/// unit is programmed). `pre_erased` ranges skip the per-unit erase.
pub struct NorSlot<'a, F> {
    flash: &'a mut F,
    region: Region,
    scratch: &'a mut [u8],
    erase_before_write: bool,
}

impl<'a, F: NorFlash> NorSlot<'a, F> {
    pub fn new(flash: &'a mut F, region: Region, scratch: &'a mut [u8]) -> Result<Self, NorError<F::Error>> {
        Self::with_mode(flash, region, scratch, true)
    }

    pub fn new_pre_erased(flash: &'a mut F, region: Region, scratch: &'a mut [u8]) -> Result<Self, NorError<F::Error>> {
        Self::with_mode(flash, region, scratch, false)
    }

    fn with_mode(flash: &'a mut F, region: Region, scratch: &'a mut [u8], erase_before_write: bool) -> Result<Self, NorError<F::Error>> {
        if scratch.len() < F::ERASE_SIZE {
            return Err(NorError::ScratchTooSmall);
        }
        Ok(Self { flash, region, scratch, erase_before_write })
    }

    fn flush_block(&mut self, logical: u64, bytes: &[u8]) -> Result<(), NorError<F::Error>> {
        let erase = F::ERASE_SIZE;
        if bytes.len() > erase {
            return Err(NorError::OutOfBounds);
        }
        if logical % erase as u64 != 0 {
            return Err(NorError::Unaligned);
        }
        // The whole erase unit must lie inside the slot.
        let start = locate::<F::Error>(self.region, logical, erase as u64)?;
        let padded = bytes.len().div_ceil(F::WRITE_SIZE) * F::WRITE_SIZE;
        self.scratch[..bytes.len()].copy_from_slice(bytes);
        self.scratch[bytes.len()..padded].fill(0);
        if self.erase_before_write {
            self.flash.erase(start, start + erase as u32).map_err(NorError::Flash)?;
        }
        self.flash.write(start, &self.scratch[..padded]).map_err(NorError::Flash)
    }
}

impl<F: NorFlash> ArtifactStorage for NorSlot<'_, F> {
    type Error = NorError<F::Error>;

    fn write(&mut self, durable_offset: u64, pending: &[u8]) -> Result<u64, Self::Error> {
        let erase = F::ERASE_SIZE;
        let mut consumed = 0usize;
        while pending.len() - consumed >= erase {
            self.flush_block(durable_offset + consumed as u64, &pending[consumed..consumed + erase])?;
            consumed += erase;
        }
        Ok(durable_offset + consumed as u64)
    }

    fn finish(&mut self, durable_offset: u64, pending: &[u8]) -> Result<u64, Self::Error> {
        if pending.is_empty() {
            return Ok(durable_offset);
        }
        if pending.len() >= F::ERASE_SIZE {
            return Err(NorError::OutOfBounds);
        }
        self.flush_block(durable_offset, pending)?;
        Ok(durable_offset + pending.len() as u64)
    }
}
