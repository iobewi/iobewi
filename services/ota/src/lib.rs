//! IOBEWI transactional, resumable, A/B-safe OTA service.
//!
//! `no_std`, and independent of the ESP32, `esp-hal`, Embassy, HTTP/TLS,
//! Kubernetes/any particular deployment control plane, and any one
//! bootloader's on-flash format. What it owns:
//!
//! * a streaming, resumable artifact write with a digest computed while
//!   writing, never re-read back afterward ([`artifact`]);
//! * a staged-transaction record and its lifecycle: staged -> activating ->
//!   resolved, with every field a multi-artifact transaction will need
//!   already in place, even though v1 only ever stages one ([`transaction`]);
//! * the decision table that reconciles a staged transaction against
//!   whatever the backend reports after a restart, so the outcome of a
//!   power cut at any point is always deterministic
//!   ([`transaction::reconcile`], [`state`]);
//! * a first-install bootstrap lifecycle kept separate from normal OTA transaction state ([`bootstrap`]);
* two effect boundaries a backend implements ([`storage`]): bulk artifact
//!   bytes (streamed, not read-back-verified here) and a small transaction
//!   record (the one place this crate requires a genuinely atomic publish).
//!
//! # What is deliberately *not* here
//!
//! * `Content-Range` (or any other resume-token wire format) -- transport
//!   detail. [`artifact::resume_plan`]/[`artifact::is_complete`] take the
//!   *decoded* numbers; parsing a header into them is the caller's job.
//! * flash mechanics (sectors, erase/program, alignment/padding) -- backend
//!   detail, behind [`storage::ArtifactStorage`].
//! * bootloader execution, partition lookup, flash I/O, MMU/cache,
//!   watchdogs, or SoC memory geometry -- those belong to the platform layer.
//!   The EWBT encoding and ESP image validator live in `iobewi-esp-ota-boot`.
//! * a decision of *what* a "target" is (a flash slot, a container tag, a
//!   block device, ...) -- opaque to this crate, carried as the `Target`
//!   type parameter of [`transaction::TransactionRecord`].
//!
//! # Stability
//!
//! Nothing in this crate is a stable API yet: every public enum is
//! `#[non_exhaustive]` and every signature should be expected to move as
//! the multi-artifact and rollback paths are built out on top of it.
#![no_std]

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod artifact;
pub mod bootstrap;
pub mod error;
#[cfg(feature = "http")]
pub mod http;
pub mod metadata;
pub mod service;
pub mod state;
pub mod storage;
pub mod transaction;

pub use artifact::{Committed, Digest, ResumePlan, WriteSession, is_complete, resume_plan};
pub use error::Error;
pub use state::{Action, BackendOutcome, TransactionState};
pub use storage::{ArtifactStorage, TransactionMetadata};
pub use transaction::{ArtifactRecord, TransactionRecord, activate, clear_stale, finish, reconcile};
