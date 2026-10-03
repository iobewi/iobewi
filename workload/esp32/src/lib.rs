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
use embedded_storage::nor_flash::{ErrorType, NorFlash};
use iobewi_esp_partitions::{PartitionError, PartitionRange, TABLE_BUFFER_SIZE, find_by_label, for_each_entry};
use iobewi_esp_flash::{EspFlash, SharedFlash};
use iobewi_workload_ota::flash::{FlashAccess, WorkloadFlash};
use iobewi_workload_ota::layout::{
    LABEL_META, LABEL_SLOT_A, LABEL_SLOT_B, Region, Unsupported, WorkloadLayout, assemble,
};
use iobewi_workload_ota::nor::NorError;
use iobewi_workload_ota::service::Availability;

#[cfg(feature = "native")]
pub mod native;

pub use iobewi_update_model as model;
pub use iobewi_workload_ota as engine;
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

/// Reads the table (through `iobewi-esp-partitions`) and assembles the Workload
/// layout. A Workload partition that overlaps any other partition makes the whole
/// device `Unsupported`.
pub async fn probe(flash: &SharedFlash) -> Capability {
    let mut buffer = Box::new([0u8; TABLE_BUFFER_SIZE]);
    let mut guard = flash.lock().await;
    let storage = guard.storage();
    let region_of = |range: PartitionRange| Region::new(range.offset, range.size as u32);
    let mut find = |label: &str| match find_by_label(storage, &mut buffer, label, TYPE_DATA, SUBTYPE_UNDEFINED) {
        Ok(range) => Ok(Some(region_of(range))),
        Err(PartitionError::NotFound) => Ok(None),
        Err(_) => Err(()),
    };
    let (Ok(meta), Ok(a), Ok(b)) = (find(LABEL_META), find(LABEL_SLOT_A), find(LABEL_SLOT_B)) else {
        return Capability::TableUnreadable;
    };
    let layout = match assemble(meta, a, b, <EspFlash as NorFlash>::ERASE_SIZE as u32) {
        Ok(layout) => layout,
        Err(why) => return Capability::Unsupported(why),
    };
    // None of the Workload regions may overlap another partition (Agent slots,
    // otadata, nvs...). Entries that are the Workload regions themselves are skipped.
    let mut overlap = false;
    let regions = layout.regions();
    if for_each_entry(storage, &mut buffer, |entry| {
        let other = Region::new(entry.offset, entry.size);
        if !regions.contains(&other) && regions.iter().any(|r| r.overlaps(&other)) {
            overlap = true;
        }
    })
    .is_err()
    {
        return Capability::TableUnreadable;
    }
    if overlap {
        return Capability::Unsupported(Unsupported::Overlap);
    }
    Capability::Supported(layout)
}

/// The platform half of the portable [`FlashAccess`]: the one process-wide
/// `SharedFlash` mutex, taken for exactly one closure.
#[derive(Clone, Copy)]
pub struct EspFlashAccess(&'static SharedFlash);

impl EspFlashAccess {
    pub fn new(flash: &'static SharedFlash) -> Self {
        Self(flash)
    }
}

impl FlashAccess for EspFlashAccess {
    type Flash = EspFlash;

    async fn with<R>(&self, f: impl FnOnce(&mut EspFlash) -> R) -> R {
        let mut guard = self.0.lock().await;
        f(&mut guard)
    }
}

/// Workload storage on an ESP device: the portable storage over the shared flash lock.
pub type EspWorkloadStorage = WorkloadFlash<EspFlashAccess>;

/// The `Availability` the portable Workload OTA service needs, from a probe.
pub fn availability(flash: &'static SharedFlash, capability: Capability) -> Availability<EspFlashAccess> {
    match capability {
        Capability::Supported(layout) => Availability::Supported(WorkloadFlash::new(layout, EspFlashAccess::new(flash))),
        Capability::Unsupported(why) => Availability::Unsupported(why),
        Capability::TableUnreadable => Availability::TableUnreadable,
    }
}

/// Erase-ahead block size, re-exported for logs/diagnostics.
pub const fn erase_batch_size() -> u64 {
    iobewi_workload_ota::flash::ERASE_BATCH
}
