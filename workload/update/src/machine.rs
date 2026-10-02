//! The Workload A/B state machine over the OTM2 store.
//!
//! Selection only: it records which Workload is active, which candidate is
//! staged and which one to return to. It does not load, run or watch a
//! Workload -- the effects go through [`WorkloadSupervisor`] /
//! [`WorkloadActivator`], implemented elsewhere.
//!
//! | state | event | next | |
//! |---|---|---|---|
//! | Empty / Valid / Staged | `commit_staged` (after `prepare` + verified write) | Staged | allowed (a Staged candidate is superseded) |
//! | Activating / PendingConfirmation / RollingBack | `prepare` | – | refused `Busy` |
//! | Staged | `activate` (runtime API satisfied) | Activating → PendingConfirmation | allowed |
//! | Staged | `activate` (runtime API not satisfied) | Staged | refused `IncompatibleRuntimeApi` |
//! | PendingConfirmation | `confirm` | Valid | allowed |
//! | Activating / PendingConfirmation / RollingBack | `rollback` | RollingBack → Valid or Empty | allowed |
//! | any other | `activate` / `confirm` / `rollback` | – | refused |

use iobewi_ota::{ArtifactStorage, Committed, Digest, WriteSession};
use iobewi_update_model::{Refusal, RuntimeApi, Side, UpdateRequest, UpdateTarget, WorkloadSupervisor};

use crate::otm2::{FieldError, Record, SlotMeta, State, slot_index};
use crate::store::{Loaded, MetadataBackend, MetadataStore};

/// Supervisor effects the engine needs: switching and restoring (from
/// [`WorkloadSupervisor`]) plus stopping a failed Workload when there is
/// nothing to return to.
pub trait WorkloadActivator: WorkloadSupervisor {
    fn stop(&mut self);
}

/// Where a restarted Agent stands (no loader involved).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    /// No Workload and no candidate.
    NoWorkload,
    Valid(Side),
    Staged { active: Option<Side>, candidate: Side },
    /// The candidate was running unconfirmed: resume the health window or roll back.
    PendingConfirmation(Side),
    /// An interrupted activation or rollback: `rollback` must be completed
    /// (`restore` is the Workload to return to, `None` = stay empty).
    RollbackRequired { restore: Option<Side> },
    /// Neither metadata copy is valid; no state is invented.
    CorruptedMetadata,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateError<E> {
    Backend(E),
    Refused(Refusal),
    /// The state does not allow this operation.
    WrongState(Option<State>),
    /// Metadata is corrupted: refuse to act until recovered explicitly.
    Corrupted,
    TooLarge,
    Field(FieldError),
    /// `commit_staged` was given a write result that is not the prepared one.
    NotPrepared,
}

/// A slot reserved for an incoming Workload, with the metadata that will be
/// persisted once the write is verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prepared {
    pub slot: Side,
    pub meta: SlotMeta,
}

impl Prepared {
    /// A streaming session for this artifact (shared engine: digest while
    /// writing, resume, `finish` verifies size and SHA-256).
    pub fn session(&self) -> WriteSession {
        WriteSession::begin(u64::from(self.meta.size), Digest(self.meta.digest))
    }
}

pub struct WorkloadUpdater<B> {
    store: MetadataStore<B>,
}

impl<B: MetadataBackend> WorkloadUpdater<B> {
    pub fn new(backend: B) -> Self {
        Self { store: MetadataStore::new(backend) }
    }

    pub fn store(&self) -> &MetadataStore<B> {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut MetadataStore<B> {
        &mut self.store
    }

    /// The current record, `None` when nothing was ever stored.
    fn current(&mut self) -> Result<Option<Record>, UpdateError<B::Error>> {
        match self.store.load().map_err(UpdateError::Backend)? {
            Loaded::Blank => Ok(None),
            Loaded::Record { record, .. } => Ok(Some(record)),
            Loaded::Corrupted => Err(UpdateError::Corrupted),
        }
    }

    fn base(current: &Option<Record>) -> Record {
        current.unwrap_or(Record::empty(0))
    }

    fn put(&mut self, mut record: Record, previous_sequence: u32) -> Result<(), UpdateError<B::Error>> {
        record.sequence = previous_sequence.wrapping_add(1);
        self.store.commit(&record).map_err(UpdateError::Backend)
    }

    pub fn state(&mut self) -> Result<Option<State>, UpdateError<B::Error>> {
        Ok(self.current()?.map(|r| r.state))
    }

    pub fn record(&mut self) -> Result<Option<Record>, UpdateError<B::Error>> {
        self.current()
    }

    /// Explicit recovery from [`Recovery::CorruptedMetadata`] (or an OTM2 factory
    /// reset): commits an `Empty` record with a fresh sequence. The Agent chooses
    /// to call it; nothing does so silently.
    pub fn format(&mut self) -> Result<(), B::Error> {
        let sequence = match self.store.load()? {
            Loaded::Record { record, .. } => record.sequence.wrapping_add(1),
            _ => 1,
        };
        self.store.commit(&Record::empty(sequence))
    }

    /// What the Agent must do after a restart.
    pub fn recover(&mut self) -> Result<Recovery, B::Error> {
        Ok(match self.store.load()? {
            Loaded::Blank => Recovery::NoWorkload,
            Loaded::Corrupted => Recovery::CorruptedMetadata,
            Loaded::Record { record, .. } => match record.state {
                State::Empty => Recovery::NoWorkload,
                State::Valid => Recovery::Valid(record.active.unwrap_or(Side::A)),
                State::Staged => Recovery::Staged {
                    active: record.active,
                    candidate: record.candidate.unwrap_or(Side::A),
                },
                State::PendingConfirmation => Recovery::PendingConfirmation(record.active.unwrap_or(Side::A)),
                State::Activating => Recovery::RollbackRequired { restore: record.active },
                State::RollingBack => Recovery::RollbackRequired { restore: record.previous_valid },
            },
        })
    }

    /// Reserves the inactive slot for `request`. Refused while an activation,
    /// a pending confirmation or a rollback is in flight; a `Staged` candidate
    /// is superseded.
    pub fn prepare(&mut self, request: &UpdateRequest, capacity: u64) -> Result<Prepared, UpdateError<B::Error>> {
        let iobewi_update_model::Compatibility::Workload { requires } = request.compatibility() else {
            return Err(UpdateError::Refused(Refusal::WrongTarget));
        };
        debug_assert_eq!(request.target(), UpdateTarget::Workload);
        let current = self.current()?;
        if let Some(record) = current {
            if matches!(record.state, State::Activating | State::PendingConfirmation | State::RollingBack) {
                return Err(UpdateError::Refused(Refusal::Busy));
            }
        }
        let artifact = request.artifact();
        if artifact.size > capacity {
            return Err(UpdateError::TooLarge);
        }
        let meta = SlotMeta::new(&artifact.id, &artifact.version, artifact.digest, artifact.size, requires)
            .map_err(UpdateError::Field)?;
        let active = Self::base(&current).active;
        Ok(Prepared { slot: active.map_or(Side::A, Side::other), meta })
    }

    /// Persists `Staged` once the artifact is fully written and its digest
    /// verified (`committed` comes from the shared `WriteSession::finish`).
    pub fn commit_staged(&mut self, prepared: &Prepared, committed: &Committed) -> Result<(), UpdateError<B::Error>> {
        if committed.digest.0 != prepared.meta.digest || committed.size != u64::from(prepared.meta.size) {
            return Err(UpdateError::NotPrepared);
        }
        let current = self.current()?;
        let mut record = Self::base(&current);
        if matches!(record.state, State::Activating | State::PendingConfirmation | State::RollingBack) {
            return Err(UpdateError::Refused(Refusal::Busy));
        }
        record.state = State::Staged;
        record.candidate = Some(prepared.slot);
        record.previous_valid = None;
        record.meta[slot_index(prepared.slot)] = prepared.meta;
        self.put(record, Self::base(&current).sequence)
    }

    /// The Workload requirement of the active slot, for the Agent to verify
    /// at boot / after its own update.
    pub fn active_requirement(&mut self) -> Result<Option<RuntimeApi>, UpdateError<B::Error>> {
        Ok(self.current()?.and_then(|r| r.active.map(|s| r.meta[slot_index(s)].requires)))
    }

    /// Everything `activate` checks **before** it persists anything or calls a
    /// supervisor, with no side effect: the state must be `Staged` and the
    /// candidate's runtime API requirement must be met. Returns the record.
    /// A caller with no supervisor to call uses this and stops there, leaving
    /// the state `Staged` -- it never pretends an activation happened.
    pub fn preflight_activate(&mut self, agent_api: RuntimeApi) -> Result<Record, UpdateError<B::Error>> {
        let current = self.current()?;
        let Some(record) = current.filter(|r| r.state == State::Staged) else {
            return Err(UpdateError::WrongState(current.map(|r| r.state)));
        };
        let candidate = record.candidate.unwrap_or(Side::A);
        if !agent_api.satisfies(record.meta[slot_index(candidate)].requires) {
            return Err(UpdateError::Refused(Refusal::IncompatibleRuntimeApi));
        }
        Ok(record)
    }

    /// Activates the staged candidate. The compatibility check runs **before**
    /// anything is persisted or the supervisor is called.
    pub fn activate<S: WorkloadSupervisor>(&mut self, supervisor: &mut S, agent_api: RuntimeApi) -> Result<(), UpdateError<B::Error>> {
        let record = self.preflight_activate(agent_api)?;
        let candidate = record.candidate.unwrap_or(Side::A);
        // 1. intent
        let mut activating = record;
        activating.state = State::Activating;
        self.put(activating, record.sequence)?;
        // 2. effect (supervisor authority, never the bootloader)
        supervisor.switch_to(candidate);
        // 3. switched, awaiting confirmation: the old active is the way back.
        let mut pending = activating;
        pending.state = State::PendingConfirmation;
        pending.previous_valid = record.active;
        pending.active = Some(candidate);
        pending.candidate = None;
        self.put(pending, activating.sequence.wrapping_add(1))
    }

    /// The trigger (health) is a future capability; this only records it.
    pub fn confirm(&mut self) -> Result<(), UpdateError<B::Error>> {
        let current = self.current()?;
        let Some(record) = current.filter(|r| r.state == State::PendingConfirmation) else {
            return Err(UpdateError::WrongState(current.map(|r| r.state)));
        };
        let mut valid = record;
        valid.state = State::Valid;
        valid.previous_valid = None;
        self.put(valid, record.sequence)
    }

    /// Returns to the last confirmed Workload (or to none), without touching
    /// the Agent. Also completes a rollback interrupted by a restart.
    pub fn rollback<A: WorkloadActivator>(&mut self, activator: &mut A) -> Result<(), UpdateError<B::Error>> {
        let current = self.current()?;
        let Some(record) = current.filter(|r| {
            matches!(r.state, State::Activating | State::PendingConfirmation | State::RollingBack)
        }) else {
            return Err(UpdateError::WrongState(current.map(|r| r.state)));
        };
        let (failed, restore) = match record.state {
            State::Activating => (record.candidate, record.active),
            _ => (record.active, record.previous_valid),
        };
        // 1. intent (skipped when resuming an interrupted rollback)
        let mut sequence = record.sequence;
        if record.state != State::RollingBack {
            let mut rolling = record;
            rolling.state = State::RollingBack;
            rolling.active = failed;
            rolling.candidate = None;
            rolling.previous_valid = restore;
            self.put(rolling, sequence)?;
            sequence = sequence.wrapping_add(1);
        }
        // 2. effect
        match restore {
            Some(side) => activator.restore(side),
            None => activator.stop(),
        }
        // 3. settled
        let mut done = record;
        done.candidate = None;
        done.previous_valid = None;
        match restore {
            Some(side) => {
                done.state = State::Valid;
                done.active = Some(side);
            }
            None => {
                done.state = State::Empty;
                done.active = None;
            }
        }
        self.put(done, sequence)
    }
}

/// Convenience for callers/tests: write `data` to a slot through the shared
/// streaming engine and return the verified result.
pub fn write_all<S: ArtifactStorage>(
    prepared: &Prepared,
    storage: &mut S,
    data: &[u8],
    chunk: usize,
) -> Result<Committed, iobewi_ota::Error<S::Error>> {
    let mut session = prepared.session();
    for piece in data.chunks(chunk.max(1)) {
        session.append(storage, piece)?;
    }
    session.finish(storage)
}
