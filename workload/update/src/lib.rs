#![no_std]

//! Workload OTA (S15): the persistent state of the *Workload* update level
//! (see `docs/dual-ota.md`) and the engine that drives it. Agent-driven: the
//! running Agent writes the inactive Workload slot, verifies it, activates it
//! through a [`WorkloadSupervisor`](iobewi_update_model::WorkloadSupervisor)
//! (no Agent reboot), then confirms or rolls back.
//!
//! * [`otm2`] -- the OTM2 binary record (codec only: layout, CRC, integrity).
//! * [`store`] -- portable double-copy metadata store (newest valid copy wins).
//! * [`machine`] -- the Workload A/B state machine, upload glue and recovery.
//! * [`layout`] -- the physical Workload regions (meta + two slots), capability
//!   (supported / unsupported) and partition-table sanity checks; pure.
//! * [`nor`] -- a generic NOR-flash backend (`embedded-storage`) for the metadata
//!   copies and the slots, with bounds checking; no platform type.
//!
//! OTM2 is **independent** of OTM1 (the Agent's record, `iobewi-ota`): other
//! magic, other storage, no shared transaction, no migration. Nothing here
//! loads or executes a Workload, measures its health, or knows a platform; the
//! physical slot (A/B) never leaves the device -- `embewi-core` only ever names
//! a target and an artifact.
//!
//! The streaming write, digest and resume mechanics are the common engine's
//! (`iobewi_ota::WriteSession`); only the activation policy differs from the
//! Agent's.

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod layout;
pub mod machine;
pub mod nor;
pub mod otm2;
pub mod store;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_nor;
