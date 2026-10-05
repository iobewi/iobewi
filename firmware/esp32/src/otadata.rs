//! ESP flash adapter for IOBEWI OTA's EWBT boot entries.
//!
//! IOBEWI OTA decides what to boot and which entry transition to make. This
//! module locates the ESP `otadata` partition and executes those transitions
//! with readback verification. The caller owns and locks the physical flash.

use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_bootloader_esp_idf::partitions::{
    DataPartitionSubType, PARTITION_TABLE_MAX_LEN, PartitionType, read_partition_table,
};
use iobewi_ota::BackendOutcome;
use iobewi_firmware_boot::{self as boot, Decoded};

use crate::{AppPartition, AppSlot, find_app_partition};
use iobewi_esp_partitions::FlashStorage;

const SLOT_COUNT: u8 = 2;
const SECTOR_SIZE: u32 = 0x1000;
pub type Entries = [boot::Raw; SLOT_COUNT as usize];
pub type TableBuffer = [u8; PARTITION_TABLE_MAX_LEN];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unavailable,
    NoTransition,
    Verify,
}

fn read_at(
    flash: &mut FlashStorage<'_>,
    scratch: &mut TableBuffer,
) -> Result<(u32, Entries), Error> {
    let table = read_partition_table(flash, scratch).map_err(|_| Error::Unavailable)?;
    let partition = table
        .find_partition(PartitionType::Data(DataPartitionSubType::Ota))
        .map_err(|_| Error::Unavailable)?
        .ok_or(Error::Unavailable)?;
    let base = partition.offset();
    let mut entries = [boot::BLANK; SLOT_COUNT as usize];
    for (i, raw) in entries.iter_mut().enumerate() {
        ReadNorFlash::read(flash, base + i as u32 * SECTOR_SIZE, raw).map_err(|_| Error::Unavailable)?;
    }
    Ok((base, entries))
}

pub fn read_entries(
    flash: &mut FlashStorage<'_>,
    scratch: &mut TableBuffer,
) -> Result<Entries, Error> {
    read_at(flash, scratch).map(|(_, entries)| entries)
}

/// Reports the running partition, including after the bootloader falls back
/// from an invalid image without rewriting the latest EWBT entry.
pub fn booted_slot(flash: &mut FlashStorage<'_>, scratch: &mut TableBuffer) -> Result<&'static str, Error> {
    let table = read_partition_table(flash, scratch).map_err(|_| Error::Unavailable)?;
    let entry = table.booted_partition().map_err(|_| Error::Unavailable)?.ok_or(Error::Unavailable)?;
    Ok(match entry.label_as_str() {
        "ota_0" => "ota_0",
        "ota_1" => "ota_1",
        "factory" => "factory",
        _ => return Err(Error::Unavailable),
    })
}

/// Choose the other slot only after IOBEWI OTA entries identify a running Valid
/// or Pending image. A stale Valid entry may remain in the other sector.
pub fn write_target(
    flash: &mut FlashStorage<'_>,
    scratch: &mut TableBuffer,
) -> Result<AppPartition, Error> {
    let entries = read_entries(flash, scratch)?;
    let target = boot::update_target(entries, SLOT_COUNT)
        .and_then(AppSlot::from_index)
        .ok_or(Error::Unavailable)?;
    find_app_partition(flash, scratch, target).map_err(|_| Error::Unavailable)
}

pub fn image_outcome(entries: &Entries) -> BackendOutcome {
    if boot::has_entry_in_state(*entries, boot::state::PENDING_VERIFY) {
        BackendOutcome::PendingConfirmation
    } else if boot::has_entry_in_state(*entries, boot::state::VALID) {
        BackendOutcome::Confirmed
    } else {
        BackendOutcome::Other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootEntry {
    pub slot: &'static str,
    pub seq: u32,
    pub state: &'static str,
}

pub fn boot_entry(entries: &Entries) -> Option<BootEntry> {
    let entry = boot::newest_entry(*entries)?;
    Some(BootEntry {
        slot: match boot::slot_of(entry.seq, SLOT_COUNT) { 0 => "ota_0", 1 => "ota_1", _ => "" },
        seq: entry.seq,
        state: match entry.state {
            boot::state::NEW => "new",
            boot::state::PENDING_VERIFY => "pending_verify",
            boot::state::VALID => "valid",
            boot::state::INVALID => "invalid",
            boot::state::ABORTED => "aborted",
            _ => "unknown",
        },
    })
}

fn execute(
    flash: &mut FlashStorage<'_>,
    scratch: &mut TableBuffer,
    write: boot::Write,
) -> Result<(), Error> {
    let (partition_base, _) = read_at(flash, scratch)?;
    let base = partition_base + u32::from(write.sector) * SECTOR_SIZE;
    let [erase, body, commit] = write.ops();
    let mut back = [0u8; boot::ENTRY_SIZE];

    let boot::Op::Erase { .. } = erase else { return Err(Error::Verify) };
    flash.erase(base, base + SECTOR_SIZE).map_err(|_| Error::Verify)?;
    ReadNorFlash::read(flash, base, &mut back).map_err(|_| Error::Verify)?;
    if back != boot::BLANK { return Err(Error::Verify); }

    let boot::Op::Program { offset, len, data, .. } = body else { return Err(Error::Verify) };
    NorFlash::write(flash, base + u32::from(offset), &data[..usize::from(len)]).map_err(|_| Error::Verify)?;
    ReadNorFlash::read(flash, base, &mut back).map_err(|_| Error::Verify)?;
    if back != write.entry.body() { return Err(Error::Verify); }

    let boot::Op::Program { offset, len, data, .. } = commit else { return Err(Error::Verify) };
    NorFlash::write(flash, base + u32::from(offset), &data[..usize::from(len)]).map_err(|_| Error::Verify)?;
    ReadNorFlash::read(flash, base, &mut back).map_err(|_| Error::Verify)?;
    if back != write.entry.encode() || boot::decode(&back) != Decoded::Ok(write.entry) {
        return Err(Error::Verify);
    }
    Ok(())
}

pub fn confirm(flash: &mut FlashStorage<'_>, scratch: &mut TableBuffer) -> Result<(), Error> {
    let entries = read_entries(flash, scratch)?;
    let write = boot::confirm(entries).ok_or(Error::NoTransition)?;
    execute(flash, scratch, write)
}

pub fn reject(flash: &mut FlashStorage<'_>, scratch: &mut TableBuffer) -> Result<(), Error> {
    let entries = read_entries(flash, scratch)?;
    let write = boot::reject(entries).ok_or(Error::NoTransition)?;
    execute(flash, scratch, write)
}

pub fn activate(
    flash: &mut FlashStorage<'_>,
    scratch: &mut TableBuffer,
    target: AppSlot,
) -> Result<(), Error> {
    let entries = read_entries(flash, scratch)?;
    let index = match target { AppSlot::Ota0 => 0, AppSlot::Ota1 => 1 };
    let write = boot::activate(entries, SLOT_COUNT, index).map_err(|_| Error::NoTransition)?;
    execute(flash, scratch, write)
}
