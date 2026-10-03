//! The Workload Supervisor: the stable layer that turns OTM2's persistent state
//! into a really running Workload, and back.
//!
//! * **OTM2** (`machine`) is the *persistent* truth: which artifact is active,
//!   staged, pending, or being rolled back.
//! * **The Supervisor** reconciles it with the *execution* state: it asks a
//!   [`WorkloadRuntime`] to start/stop an artifact and reports whether it runs and
//!   is healthy. It never writes OTM2 itself: every state change goes through the
//!   persisted steps of the Workload OTA engine, with the runtime call *between*
//!   two persisted steps and the flash lock released:
//!
//! ```text
//! activate : verify slot digest -> persist Activating -> stop old -> start new
//!            -> persist PendingConfirmation          (start fails -> rollback)
//! confirm  : PendingConfirmation && running(candidate) && Healthy -> persist Valid
//! rollback : persist RollingBack -> stop -> start previous (verified) -> persist
//!            Valid(previous) | Empty         (restore fails -> stays RollingBack)
//! boot     : reconcile(OTM2 state) -- see [`WorkloadSupervisor::reconcile_boot`]
//! ```
//!
//! The runtime behind [`WorkloadRuntime`] is the part that will change (a Wasm or
//! native loader later, a validation probe today); the Supervisor, its states and
//! OTM2 do not. Exactly one Workload can run. Stopping the old Workload before
//! starting the new one is not CPU-atomic; atomicity is transactional (OTM2 plus
//! rollback), not instantaneous.

use alloc::string::String;
use core::cell::Cell;

use iobewi_update_model::{RuntimeApi, Side};

use crate::flash::{FlashAccess, WorkloadFlash};
use crate::machine::{Recovery, UpdateError};
use crate::otm2::{Record, State, slot_index};
use crate::service::{ServiceError, WorkloadOtaService};

/// Which artifact is (or should be) running: everything the runtime and the
/// confirmation need to recognise it. No slot is ever exposed outside the device;
/// `slot` is local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub slot: Side,
    pub id: String,
    pub version: String,
    pub digest: [u8; 32],
    pub size: u32,
    pub requires: RuntimeApi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Unhealthy,
    /// Nothing to judge (no Workload running, or the runtime cannot tell).
    Unknown,
}

impl Health {
    pub const fn as_str(self) -> &'static str {
        match self {
            Health::Healthy => "healthy",
            Health::Unhealthy => "unhealthy",
            Health::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeError {
    pub reason: &'static str,
}

/// Read access to the running artifact's bytes, without loading it in RAM and
/// without holding the flash: each read takes the lock for that read only.
#[allow(async_fn_in_trait)]
pub trait ArtifactReader {
    async fn read(&self, offset: u64, buf: &mut [u8]) -> Result<(), RuntimeError>;
}

/// The execution backend. One Workload at a time; `start` only returns `Ok` once
/// the Workload is running, `stop` only returns once it has really stopped (no
/// zombie task), and both are idempotent.
#[allow(async_fn_in_trait)]
pub trait WorkloadRuntime {
    /// Can this runtime run `artifact`? Called by `activate` *before* anything is persisted
    /// or stopped, so a refusal leaves the candidate `Staged` and the active Workload
    /// untouched. Must have no side effect (no load, no execution). The default accepts
    /// everything (the S18 probe).
    async fn preflight<R: ArtifactReader>(&self, _artifact: &Identity, _reader: &R) -> Result<(), RuntimeError> {
        Ok(())
    }
    async fn start<R: ArtifactReader>(&self, artifact: &Identity, reader: &R) -> Result<(), RuntimeError>;
    async fn stop(&self);
    async fn health(&self) -> Health;
    /// What is running right now, if anything.
    async fn running(&self) -> Option<Identity>;
}

/// What `reconcile_boot` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootOutcome {
    /// Workload storage is not supported on this device.
    Unsupported,
    /// Nothing to run.
    Idle,
    /// The recorded Workload is running.
    Started(Side),
    /// The recorded Workload could not be started (state unchanged).
    StartFailed,
    /// An unfinished activation/confirmation/rollback was resolved by rolling back;
    /// `restored` is the Workload running again (`None` = none).
    RolledBack { restored: Option<Side> },
    /// A rollback could not restore the previous Workload: state stays `RollingBack`.
    RollbackFailed,
    /// OTM2 metadata is unreadable: nothing is invented, nothing runs.
    Corrupted,
}

/// Execution status next to OTM2's persistent status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStatus {
    pub running: Option<Identity>,
    pub health: Health,
}

struct BusyGuard<'a>(&'a Cell<bool>);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

pub struct WorkloadSupervisor<A: FlashAccess + 'static, R> {
    service: &'static WorkloadOtaService<A>,
    runtime: R,
    busy: Cell<bool>,
}

struct SlotReader<'a, A: FlashAccess> {
    flash: &'a WorkloadFlash<A>,
    slot: Side,
}

impl<A: FlashAccess> ArtifactReader for SlotReader<'_, A> {
    async fn read(&self, offset: u64, buf: &mut [u8]) -> Result<(), RuntimeError> {
        self.flash
            .read_slot(self.slot, offset, buf)
            .await
            .map_err(|_| RuntimeError { reason: "slot read failed" })
    }
}

fn identity_of(record: &Record, side: Side) -> Identity {
    let meta = &record.meta[slot_index(side)];
    Identity {
        slot: side,
        id: String::from(meta.id()),
        version: String::from(meta.version()),
        digest: meta.digest,
        size: meta.size,
        requires: meta.requires,
    }
}

fn storage_error<E>(error: UpdateError<E>) -> ServiceError {
    match error {
        UpdateError::Corrupted => ServiceError::Corrupted,
        UpdateError::WrongState(state) => ServiceError::WrongState(state),
        UpdateError::Refused(iobewi_update_model::Refusal::Busy) => ServiceError::TransitionInProgress,
        _ => ServiceError::Storage,
    }
}

impl<A: FlashAccess, R: WorkloadRuntime> WorkloadSupervisor<A, R> {
    pub fn new(service: &'static WorkloadOtaService<A>, runtime: R) -> Self {
        Self { service, runtime, busy: Cell::new(false) }
    }

    pub fn service(&self) -> &'static WorkloadOtaService<A> {
        self.service
    }

    pub fn runtime(&self) -> &R {
        &self.runtime
    }

    fn guard(&self) -> Result<BusyGuard<'_>, ServiceError> {
        if self.busy.replace(true) {
            return Err(ServiceError::TransitionInProgress);
        }
        Ok(BusyGuard(&self.busy))
    }

    pub async fn runtime_status(&self) -> RuntimeStatus {
        let running = self.runtime.running().await;
        // Ask even when nothing "runs": a Workload that died or was quarantined is `Unhealthy`,
        // not "unknown" (a runtime with nothing loaded answers `Unknown` itself).
        let health = self.runtime.health().await;
        RuntimeStatus { running, health }
    }

    /// Verifies the slot against OTM2's digest (read back from flash, never trusting
    /// that the write once succeeded), then asks the runtime to start it.
    async fn start_slot(&self, flash: &WorkloadFlash<A>, record: &Record, side: Side) -> Result<(), ()> {
        let meta = &record.meta[slot_index(side)];
        match flash.read_digest(side, meta.size).await {
            Ok(digest) if digest == meta.digest => {}
            _ => return Err(()),
        }
        let identity = identity_of(record, side);
        self.runtime.start(&identity, &SlotReader { flash, slot: side }).await.map_err(|_| ())
    }

    /// Rollback shared by manual rollback, automatic rollback and boot recovery.
    /// The previous Workload's metadata is untouched until it runs again.
    async fn rollback_inner(&self, flash: &WorkloadFlash<A>) -> Result<Option<Side>, ServiceError> {
        let plan = flash.begin_rollback().await.map_err(storage_error)?;
        self.runtime.stop().await;
        if let Some(previous) = plan.restore {
            let record = match flash.record().await {
                Ok(Some(record)) => record,
                _ => return Err(ServiceError::RollbackFailed),
            };
            if self.start_slot(flash, &record, previous).await.is_err() {
                return Err(ServiceError::RollbackFailed);
            }
        }
        flash.complete_rollback().await.map_err(storage_error)?;
        Ok(plan.restore)
    }

    /// `Staged -> Activating -> (start) -> PendingConfirmation`. A start failure
    /// rolls back automatically; the error says whether it did.
    pub async fn activate(&self, expected: &[u8; 32]) -> Result<(), ServiceError> {
        let _busy = self.guard()?;
        let record = self.service.check_activation(Some(expected)).await?;
        let flash = self.service.storage()?;
        let candidate = record.candidate.ok_or(ServiceError::WrongState(Some(State::Staged)))?;
        let meta = &record.meta[slot_index(candidate)];

        // The slot must still hold what OTM2 says; the active Workload keeps running meanwhile.
        match flash.read_digest(candidate, meta.size).await {
            Ok(digest) if digest == meta.digest => {}
            Ok(_) => {
                let _ = flash.begin_overwrite().await; // an unusable candidate stops existing
                return Err(ServiceError::CandidateCorrupted);
            }
            Err(_) => return Err(ServiceError::Storage),
        }

        let identity = identity_of(&record, candidate);
        // The runtime may refuse this artifact (wrong target, unknown format...): nothing
        // has been persisted or stopped yet, so the candidate simply stays Staged.
        if let Err(e) = self.runtime.preflight(&identity, &SlotReader { flash, slot: candidate }).await {
            return Err(ServiceError::ImageRejected(e.reason));
        }

        flash.begin_activation(self.service.provided_runtime_api()).await.map_err(storage_error)?;
        self.runtime.stop().await; // one Workload at a time: stop the old one first
        match self.runtime.start(&identity, &SlotReader { flash, slot: candidate }).await {
            Ok(()) => flash.complete_activation().await.map_err(storage_error),
            Err(_) => match self.rollback_inner(flash).await {
                Ok(_) => Err(ServiceError::ActivationFailed),
                Err(e) => Err(e),
            },
        }
    }

    /// `PendingConfirmation -> Valid`, only if the candidate really runs and is healthy.
    pub async fn confirm(&self) -> Result<(), ServiceError> {
        let _busy = self.guard()?;
        let flash = self.service.storage()?;
        let record = match flash.record().await {
            Ok(Some(record)) => record,
            Ok(None) => return Err(ServiceError::WrongState(None)),
            Err(e) => return Err(storage_error(e)),
        };
        if record.state != State::PendingConfirmation {
            return Err(ServiceError::WrongState(Some(record.state)));
        }
        let Some(active) = record.active else { return Err(ServiceError::WrongState(Some(record.state))) };
        let expected = identity_of(&record, active);
        match self.runtime.running().await {
            Some(running) if running == expected => {}
            _ => return Err(ServiceError::NotRunning),
        }
        match self.runtime.health().await {
            Health::Healthy => {}
            other => return Err(ServiceError::Unhealthy(other)),
        }
        flash.confirm().await.map_err(storage_error)
    }

    /// Manual rollback of an unconfirmed candidate (or resume of an interrupted one).
    pub async fn rollback(&self) -> Result<(), ServiceError> {
        let _busy = self.guard()?;
        let flash = self.service.storage()?;
        match flash.record().await {
            Ok(Some(r)) if matches!(r.state, State::PendingConfirmation | State::RollingBack) => {}
            Ok(Some(r)) => return Err(ServiceError::WrongState(Some(r.state))),
            Ok(None) => return Err(ServiceError::WrongState(None)),
            Err(e) => return Err(storage_error(e)),
        }
        self.rollback_inner(flash).await.map(|_| ())
    }

    /// Boot-time reconciliation of OTM2's persistent state with the execution state.
    ///
    /// | OTM2 | action |
    /// |---|---|
    /// | none / Empty | nothing runs |
    /// | Valid(A) | start A (digest verified) |
    /// | Staged, active A | start A, the candidate stays staged |
    /// | Staged, no active | nothing runs |
    /// | Activating | roll back (never assume the candidate started) |
    /// | PendingConfirmation | roll back: a restart during probation is an unconfirmed activation |
    /// | RollingBack | resume the rollback |
    /// | corrupted | nothing runs, nothing invented |
    pub async fn reconcile_boot(&self) -> BootOutcome {
        let Ok(_busy) = self.guard() else { return BootOutcome::StartFailed };
        let Ok(flash) = self.service.storage() else { return BootOutcome::Unsupported };
        let recovery = match flash.recover().await {
            Ok(r) => r,
            Err(_) => return BootOutcome::Corrupted,
        };
        match recovery {
            Recovery::CorruptedMetadata => BootOutcome::Corrupted,
            Recovery::NoWorkload => {
                self.runtime.stop().await;
                BootOutcome::Idle
            }
            Recovery::Valid(side) | Recovery::Staged { active: Some(side), .. } => {
                let Ok(Some(record)) = flash.record().await else { return BootOutcome::Corrupted };
                match self.start_slot(flash, &record, side).await {
                    Ok(()) => BootOutcome::Started(side),
                    Err(()) => BootOutcome::StartFailed,
                }
            }
            Recovery::Staged { active: None, .. } => BootOutcome::Idle,
            Recovery::PendingConfirmation(_) | Recovery::RollbackRequired { .. } => {
                match self.rollback_inner(flash).await {
                    Ok(restored) => BootOutcome::RolledBack { restored },
                    Err(_) => BootOutcome::RollbackFailed,
                }
            }
        }
    }
}
