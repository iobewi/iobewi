//! The Workload OTA service: everything an HTTP (or any other) front end needs,
//! with no HTTP, no ESP and no supervisor in it.
//!
//! * **Capability**: [`Availability`] -- supported with a layout, or unsupported
//!   with a reason. An unsupported device answers `status` and refuses
//!   everything else; it never writes to "free" flash.
//! * **Prepare** names the artifact (id, version, size, SHA-256, required
//!   runtime API). The slot is chosen locally by OTM2; nothing physical is
//!   exposed or accepted.
//! * **Write session**: `begin` / `chunk` / `finish` over the shared streaming
//!   engine (`WriteSession`: digest while writing, size, resume watermark). Only
//!   a verified `finish` stages the candidate in OTM2.
//! * **Activation**: [`WorkloadOtaService::check_activation`] runs every check of
//!   an activation (state `Staged`, runtime API) without persisting anything; a
//!   platform with no supervisor stops there with `SupervisorUnavailable` and the
//!   state stays `Staged`. [`WorkloadOtaService::activate`] performs the real
//!   transition through a [`WorkloadSupervisor`].
//!
//! The session lives in RAM (a `RefCell`, never borrowed across an `.await`:
//! pieces are taken out, used, and put back, so a second concurrent caller sees
//! an idle/busy session instead of a double borrow). A reboot loses the session,
//! not the OTM2 state: a client re-prepares and re-writes.

use alloc::string::String;
use core::cell::RefCell;

use iobewi_ota::Error as EngineError;
use iobewi_update_model::{ArtifactDescriptor, Refusal, RuntimeApi, Side, UpdateRequest, WorkloadSupervisor};

use crate::flash::{FlashAccess, SlotWriter, StorageError, WorkloadFlash};
use crate::layout::{MIN_WORKLOAD_SLOT_SIZE, Unsupported};
use crate::machine::{Prepared, UpdateError};
use crate::otm2::{FieldError, Record, SlotMeta, State, slot_index};

/// Whether this device has Workload storage.
pub enum Availability<A> {
    Supported(WorkloadFlash<A>),
    Unsupported(Unsupported),
    /// The partition table could not be read.
    TableUnreadable,
}

/// Artifact identity as the control plane names it (no slot, no offset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareInput {
    pub artifact_id: String,
    pub version: String,
    pub size: u64,
    pub digest: [u8; 32],
    pub requires: RuntimeApi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceError {
    Unsupported(Unsupported),
    TableUnreadable,
    /// OTM2 metadata is unreadable: nothing is invented, an explicit `format` is needed.
    Corrupted,
    /// An activation / pending confirmation / rollback is in flight.
    Busy(State),
    /// The operation needs another state (`None` = nothing stored yet).
    WrongState(Option<State>),
    /// No `prepare` is pending for this `begin`.
    NotPrepared,
    /// The write's digest/size are not the prepared artifact's.
    SessionMismatch,
    /// The activation names a different candidate than the staged one.
    CandidateMismatch,
    TooLarge { max: u32 },
    EmptyArtifact,
    BadField(FieldError),
    IncompatibleRuntimeApi { required: RuntimeApi, provided: RuntimeApi },
    /// Computed digest of the received bytes.
    DigestMismatch([u8; 32]),
    Incomplete { durable: u64 },
    /// No Workload supervisor exists on this platform: activation is refused,
    /// the state is untouched.
    SupervisorUnavailable,
    /// Another activation/confirmation/rollback is running in the supervisor.
    TransitionInProgress,
    /// The candidate failed to start; the previous Workload was restored (or none).
    ActivationFailed,
    /// A rollback could not restore the previous Workload: the state stays
    /// `RollingBack` (retried at the next boot), never an invented `Valid`.
    RollbackFailed,
    /// The slot no longer matches the staged digest; the candidate was discarded.
    CandidateCorrupted,
    /// The runtime refuses this artifact (wrong target, unknown format, out of budget...).
    /// Nothing was persisted or stopped: the candidate stays `Staged`.
    ImageRejected(&'static str),
    /// The confirmed candidate is not what the supervisor is running.
    NotRunning,
    /// The running candidate does not report `Healthy`.
    Unhealthy(crate::supervisor::Health),
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedResult {
    pub written: u64,
    pub digest: [u8; 32],
}

/// One slot's artifact as `status` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInfo {
    pub id: String,
    pub version: String,
    pub digest: [u8; 32],
    pub size: u32,
    pub requires: RuntimeApi,
}

impl ArtifactInfo {
    fn of(meta: &SlotMeta) -> Self {
        Self {
            id: String::from(meta.id()),
            version: String::from(meta.version()),
            digest: meta.digest,
            size: meta.size,
            requires: meta.requires,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub supported: bool,
    /// Why not supported (a `Debug` rendering of the reason), when `supported` is false.
    pub reason: Option<String>,
    /// OTM2 state; `None` = nothing stored yet; `corrupted` metadata is flagged.
    pub state: Option<State>,
    pub corrupted: bool,
    pub active: Option<ArtifactInfo>,
    pub candidate: Option<ArtifactInfo>,
    pub previous: Option<ArtifactInfo>,
    pub max_artifact_size: u32,
    /// The runtime API the Agent provides.
    pub provided: RuntimeApi,
    /// Diagnostic only: Core never needs a slot to order anything.
    pub active_slot: Option<Side>,
    pub candidate_slot: Option<Side>,
    pub write_in_progress: bool,
    pub prepared: bool,
}

#[derive(Default)]
struct Session {
    prepared: Option<Prepared>,
    writer: Option<SlotWriter>,
    writing: bool,
    received: u64,
    durable: u64,
}

pub struct WorkloadOtaService<A> {
    availability: Availability<A>,
    provided: RuntimeApi,
    session: RefCell<Session>,
}

impl<A: FlashAccess> WorkloadOtaService<A> {
    /// `provided` is the runtime API this Agent offers to Workloads -- one
    /// source of truth, supplied by the composition root.
    pub fn new(availability: Availability<A>, provided: RuntimeApi) -> Self {
        Self { availability, provided, session: RefCell::new(Session::default()) }
    }

    /// The Workload storage, or why there is none (platform glue and tests).
    pub fn storage(&self) -> Result<&WorkloadFlash<A>, ServiceError> {
        self.flash()
    }

    pub fn provided_runtime_api(&self) -> RuntimeApi {
        self.provided
    }

    fn flash(&self) -> Result<&WorkloadFlash<A>, ServiceError> {
        match &self.availability {
            Availability::Supported(flash) => Ok(flash),
            Availability::Unsupported(why) => Err(ServiceError::Unsupported(*why)),
            Availability::TableUnreadable => Err(ServiceError::TableUnreadable),
        }
    }

    fn map_update<E>(error: &UpdateError<StorageError<A>>) -> ServiceError {
        match error {
            UpdateError::Corrupted => ServiceError::Corrupted,
            UpdateError::WrongState(state) => ServiceError::WrongState(*state),
            UpdateError::Refused(Refusal::Busy) => ServiceError::Busy(State::Activating),
            UpdateError::Refused(Refusal::IncompatibleRuntimeApi) => {
                ServiceError::IncompatibleRuntimeApi { required: RuntimeApi::new(0, 0), provided: RuntimeApi::new(0, 0) }
            }
            UpdateError::Refused(_) => ServiceError::WrongState(None),
            UpdateError::TooLarge => ServiceError::TooLarge { max: 0 },
            UpdateError::Field(f) => ServiceError::BadField(*f),
            UpdateError::NotPrepared => ServiceError::SessionMismatch,
            UpdateError::Backend(_) => ServiceError::Storage,
        }
    }

    /// TEST ONLY (`test-fault-injection`): damage one byte of the staged candidate's slot.
    #[cfg(feature = "test-fault-injection")]
    pub async fn corrupt_candidate_for_test(&self, offset: u64) -> bool {
        let Ok(flash) = self.storage() else { return false };
        match self.status().await.candidate_slot {
            Some(side) => flash.corrupt_slot_byte_for_test(side, offset).await,
            None => false,
        }
    }

    // ----- status ---------------------------------------------------------

    pub async fn status(&self) -> Status {
        let mut status = Status {
            supported: false,
            reason: None,
            state: None,
            corrupted: false,
            active: None,
            candidate: None,
            previous: None,
            max_artifact_size: 0,
            provided: self.provided,
            active_slot: None,
            candidate_slot: None,
            write_in_progress: self.session.borrow().writing,
            prepared: self.session.borrow().prepared.is_some(),
        };
        let flash = match &self.availability {
            Availability::Supported(flash) => flash,
            Availability::Unsupported(why) => {
                status.reason = Some(alloc::format!("{why:?}"));
                return status;
            }
            Availability::TableUnreadable => {
                status.reason = Some(String::from("TableUnreadable"));
                return status;
            }
        };
        status.supported = true;
        status.max_artifact_size = flash.layout().max_artifact_size();
        match flash.record().await {
            Ok(Some(record)) => Self::fill(&mut status, &record),
            Ok(None) => {}
            Err(UpdateError::Corrupted) => status.corrupted = true,
            Err(_) => status.corrupted = true,
        }
        status
    }

    fn fill(status: &mut Status, record: &Record) {
        let info = |side: Option<Side>| side.map(|s| ArtifactInfo::of(&record.meta[slot_index(s)]));
        status.state = Some(record.state);
        status.active = info(record.active);
        status.candidate = info(record.candidate);
        status.previous = info(record.previous_valid);
        status.active_slot = record.active;
        status.candidate_slot = record.candidate;
    }

    // ----- prepare / write ------------------------------------------------

    /// Reserves the inactive slot for the named artifact. Refused while an
    /// activation, a pending confirmation or a rollback is in flight; a `Staged`
    /// candidate is superseded. A previous prepared session (and any write in
    /// progress) is dropped.
    pub async fn prepare(&self, input: &PrepareInput) -> Result<(), ServiceError> {
        let flash = self.flash()?;
        if input.size == 0 {
            return Err(ServiceError::EmptyArtifact);
        }
        let request = UpdateRequest::workload(
            ArtifactDescriptor {
                id: input.artifact_id.clone(),
                version: input.version.clone(),
                digest: input.digest,
                size: input.size,
            },
            input.requires,
        );
        match flash.prepare(&request).await {
            Ok(prepared) => {
                let mut session = self.session.borrow_mut();
                *session = Session { prepared: Some(prepared), ..Session::default() };
                Ok(())
            }
            Err(UpdateError::TooLarge) => Err(ServiceError::TooLarge { max: flash.layout().max_artifact_size() }),
            Err(UpdateError::Refused(Refusal::Busy)) => {
                let state = flash.recover().await.ok().and_then(|_| None::<State>);
                let state = match flash.record().await {
                    Ok(Some(record)) => record.state,
                    _ => state.unwrap_or(State::Activating),
                };
                Err(ServiceError::Busy(state))
            }
            Err(other) => Err(Self::map_update::<()>(&other)),
        }
    }

    pub fn in_progress(&self) -> bool {
        self.session.borrow().writing
    }

    /// Bytes accepted by the session so far (what an uninterrupted client continues from).
    pub fn received(&self) -> u64 {
        self.session.borrow().received
    }

    /// Bytes durable in flash (what a client resumes from after a drop).
    pub fn written(&self) -> u64 {
        self.session.borrow().durable
    }

    /// Do `digest`/`total` name the artifact the session was prepared for?
    pub fn params_match(&self, digest: &[u8; 32], total: u64) -> bool {
        self.session
            .borrow()
            .prepared
            .as_ref()
            .is_some_and(|p| p.meta.digest == *digest && u64::from(p.meta.size) == total)
    }

    /// Starts (or restarts) the write of the prepared artifact. The client's
    /// digest and total must be the prepared ones.
    pub async fn begin(&self, digest: &[u8; 32], total: u64) -> Result<(), ServiceError> {
        let flash = self.flash()?;
        let prepared = {
            let session = self.session.borrow();
            match &session.prepared {
                None => return Err(ServiceError::NotPrepared),
                Some(p) if p.meta.digest != *digest || u64::from(p.meta.size) != total => {
                    return Err(ServiceError::SessionMismatch);
                }
                Some(p) => *p,
            }
        };
        // The first byte lands in the inactive slot: a Staged candidate stops existing here,
        // and an in-flight activation/confirmation/rollback forbids writing at all.
        match flash.begin_overwrite().await {
            Ok(()) => {}
            Err(UpdateError::Refused(Refusal::Busy)) => {
                let state = match flash.record().await {
                    Ok(Some(record)) => record.state,
                    _ => State::Activating,
                };
                return Err(ServiceError::Busy(state));
            }
            Err(other) => return Err(Self::map_update::<()>(&other)),
        }
        let writer = flash.writer(&prepared);
        let mut session = self.session.borrow_mut();
        session.writer = Some(writer);
        session.writing = true;
        session.received = 0;
        session.durable = 0;
        Ok(())
    }

    /// Appends one chunk. `false` on any failure; a failed session is dropped
    /// (the client re-prepares) and nothing is staged.
    pub async fn chunk(&self, bytes: &[u8]) -> bool {
        let Ok(flash) = self.flash() else { return false };
        let Some(mut writer) = self.session.borrow_mut().writer.take() else { return false };
        let ok = writer.append(flash.access(), bytes).await;
        let mut session = self.session.borrow_mut();
        session.received = writer.received();
        session.durable = writer.durable();
        if ok {
            session.writer = Some(writer);
        } else {
            session.writing = false;
        }
        ok
    }

    /// Verifies the complete artifact (size and SHA-256, while it was written)
    /// and only then stages it in OTM2. A wrong digest never stages anything:
    /// the active Workload is untouched.
    pub async fn finish(&self) -> Result<StagedResult, ServiceError> {
        let flash = self.flash()?;
        let (mut writer, prepared) = {
            let mut session = self.session.borrow_mut();
            match (session.writer.take(), session.prepared) {
                (Some(w), Some(p)) => (w, p),
                _ => return Err(ServiceError::NotPrepared),
            }
        };
        let result = writer.finish(flash.access()).await;
        self.session.borrow_mut().writing = false;
        let committed = match result {
            Ok(committed) => committed,
            Err(EngineError::DigestMismatch(computed)) => return Err(ServiceError::DigestMismatch(computed.0)),
            Err(EngineError::Incomplete { durable }) => return Err(ServiceError::Incomplete { durable }),
            Err(_) => return Err(ServiceError::Storage),
        };
        match flash.commit_staged(&prepared, &committed).await {
            Ok(()) => {
                *self.session.borrow_mut() = Session::default();
                Ok(StagedResult { written: committed.size, digest: committed.digest.0 })
            }
            Err(UpdateError::Refused(Refusal::Busy)) => Err(ServiceError::Busy(State::Activating)),
            Err(other) => Err(Self::map_update::<()>(&other)),
        }
    }

    // ----- activation -----------------------------------------------------

    /// Every check of an activation, persisting nothing: Workload storage is
    /// supported, the state is `Staged`, the named candidate (if any) is the
    /// staged one, and the Agent provides the runtime API it requires. A caller
    /// with no supervisor stops here and the state stays `Staged`.
    pub async fn check_activation(&self, expected: Option<&[u8; 32]>) -> Result<Record, ServiceError> {
        let flash = self.flash()?;
        let record = match flash.preflight_activate(self.provided).await {
            Ok(record) => record,
            Err(UpdateError::WrongState(state)) => return Err(ServiceError::WrongState(state)),
            Err(UpdateError::Refused(Refusal::IncompatibleRuntimeApi)) => {
                let required = match flash.record().await {
                    Ok(Some(r)) => r.candidate.map_or(RuntimeApi::new(0, 0), |s| r.meta[slot_index(s)].requires),
                    _ => RuntimeApi::new(0, 0),
                };
                return Err(ServiceError::IncompatibleRuntimeApi { required, provided: self.provided });
            }
            Err(other) => return Err(Self::map_update::<()>(&other)),
        };
        if let (Some(expected), Some(candidate)) = (expected, record.candidate) {
            if record.meta[slot_index(candidate)].digest != *expected {
                return Err(ServiceError::CandidateMismatch);
            }
        }
        Ok(record)
    }

    /// The real activation, through a supervisor. Production code without a
    /// supervisor never calls this (see [`Self::check_activation`]).
    pub async fn activate<S: WorkloadSupervisor>(&self, expected: Option<&[u8; 32]>, supervisor: &mut S) -> Result<(), ServiceError> {
        self.check_activation(expected).await?;
        let flash = self.flash()?;
        flash.activate(supervisor, self.provided).await.map_err(|e| Self::map_update::<()>(&e))
    }

    /// Size guard shared with tooling: the smallest slot the layout may offer.
    pub const MIN_SLOT_SIZE: u32 = MIN_WORKLOAD_SLOT_SIZE;
}
