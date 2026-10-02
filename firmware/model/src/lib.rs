#![no_std]

//! Dual-OTA model (S14): the portable vocabulary and the two *separate*
//! activation policies of EmbeWi's two update levels.
//!
//! * **Agent** -- `embewi-agent`, the resident runner / workload supervisor.
//!   Its update is *bootloader-driven*: the new image is written to the
//!   inactive Agent slot, activation schedules the next boot, a reboot is
//!   required, and the bootloader (plus the Agent OTA metadata) decides which
//!   Agent slot physically starts. Confirmation follows a reboot and an Agent
//!   self-check; rollback is the bootloader returning to the previous Agent.
//! * **Workload** -- the supervised application / Pod. Its update is
//!   *Agent-driven*: the running Agent writes the inactive Workload slot,
//!   verifies it, activates it (no reboot of the Agent), watches its health,
//!   then confirms or rolls back by restarting the previous Workload. The
//!   bootloader never selects a Workload.
//!
//! "Agent" is a resident runner/supervisor, **not** an operating-system
//! kernel: nothing here implies syscalls, privilege levels, an MMU/MPU or a
//! process model.
//!
//! The two levels are ordered independently by `embewi-core`, which names a
//! logical [`UpdateTarget`] plus an artifact (id, version, digest, size) and
//! a compatibility declaration -- never an A/B slot: the device chooses its
//! own inactive slot ([`AbSlots`]). Agent and Workload are *not* an atomic
//! release; either can change without the other, subject to
//! [`RuntimeApi`] compatibility.
//!
//! This crate is a model: no flash, no HTTP, no ESP, no Kubernetes types, no
//! persistence format (OTM1 stays the Agent's record, see
//! `docs/dual-ota.md`). The streaming/transactional mechanics (digest, resume,
//! supersession, reconcile) remain in `iobewi-ota`, shared by both levels.

extern crate alloc;
#[cfg(test)]
extern crate std;

use alloc::string::String;

/// What an update order is about. Exactly two targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTarget {
    /// `embewi-agent` itself (bootloader-driven, reboot required).
    Agent,
    /// The supervised workload / Pod (Agent-driven, no Agent reboot).
    Workload,
}

/// Version of the runtime API an Agent provides to Workloads (and a Workload
/// requires). Deliberately not called an ABI or a kernel interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeApi {
    pub major: u16,
    pub minor: u16,
}

impl RuntimeApi {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// An Agent providing `self` can run a Workload requiring `required`
    /// when the major versions match and the provided minor is at least the
    /// required one.
    pub const fn satisfies(self, required: RuntimeApi) -> bool {
        self.major == required.major && self.minor >= required.minor
    }
}

/// What an artifact says about itself, independent of target and storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDescriptor {
    pub id: String,
    /// Version / revision label (opaque here).
    pub version: String,
    pub digest: [u8; 32],
    pub size: u64,
}

/// Compatibility declaration carried by an artifact; it fixes the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    /// An Agent artifact declares the runtime API it provides.
    Agent { provides: RuntimeApi },
    /// A Workload artifact declares the runtime API it requires.
    Workload { requires: RuntimeApi },
}

/// A control-plane order translated into the engine's vocabulary. Built only
/// through [`UpdateRequest::agent`] / [`UpdateRequest::workload`], so the
/// target and the compatibility declaration can never disagree. It carries
/// no slot: the device picks its inactive slot locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateRequest {
    artifact: ArtifactDescriptor,
    compatibility: Compatibility,
}

impl UpdateRequest {
    pub fn agent(artifact: ArtifactDescriptor, provides: RuntimeApi) -> Self {
        Self { artifact, compatibility: Compatibility::Agent { provides } }
    }

    pub fn workload(artifact: ArtifactDescriptor, requires: RuntimeApi) -> Self {
        Self { artifact, compatibility: Compatibility::Workload { requires } }
    }

    pub fn target(&self) -> UpdateTarget {
        match self.compatibility {
            Compatibility::Agent { .. } => UpdateTarget::Agent,
            Compatibility::Workload { .. } => UpdateTarget::Workload,
        }
    }

    pub fn artifact(&self) -> &ArtifactDescriptor {
        &self.artifact
    }

    pub fn compatibility(&self) -> Compatibility {
        self.compatibility
    }
}

/// The two physical positions of an A/B pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

impl Side {
    pub const fn other(self) -> Self {
        match self {
            Side::A => Side::B,
            Side::B => Side::A,
        }
    }
}

/// An A/B slot set holding a value `T` per side (its metadata). It knows
/// which side is active and which is inactive; it carries **no activation
/// policy** -- that belongs to [`AgentOta`] / [`WorkloadOta`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbSlots<T> {
    a: Option<T>,
    b: Option<T>,
    active: Side,
}

impl<T> AbSlots<T> {
    /// A set whose active side `Side::A` already holds `installed`.
    pub fn with_active(installed: T) -> Self {
        Self { a: Some(installed), b: None, active: Side::A }
    }

    pub fn empty() -> Self {
        Self { a: None, b: None, active: Side::A }
    }

    pub fn active_side(&self) -> Side {
        self.active
    }

    pub fn inactive_side(&self) -> Side {
        self.active.other()
    }

    fn slot(&self, side: Side) -> &Option<T> {
        match side {
            Side::A => &self.a,
            Side::B => &self.b,
        }
    }

    fn slot_mut(&mut self, side: Side) -> &mut Option<T> {
        match side {
            Side::A => &mut self.a,
            Side::B => &mut self.b,
        }
    }

    pub fn active(&self) -> Option<&T> {
        self.slot(self.active).as_ref()
    }

    pub fn inactive(&self) -> Option<&T> {
        self.slot(self.inactive_side()).as_ref()
    }

    /// Writes `value` to the inactive side (superseding whatever it held).
    pub fn write_inactive(&mut self, value: T) -> Side {
        let side = self.inactive_side();
        *self.slot_mut(side) = Some(value);
        side
    }

    /// Makes `side` the active one.
    pub fn select(&mut self, side: Side) {
        self.active = side;
    }
}

/// Bootloader authority over Agent slots: the only thing an Agent update may
/// act on. (Implemented by the platform boot layer, e.g. EWBT on ESP.)
pub trait BootAuthority {
    /// Schedule `side` to be tried at the next boot (reboot still required).
    fn schedule_boot(&mut self, side: Side);
    /// Restore `side` as the Agent to boot (after a failed/unconfirmed one).
    fn restore_boot(&mut self, side: Side);
}

/// Supervisor authority over Workload slots: the only thing a Workload update
/// may act on. (Implemented by the Agent's workload supervisor; never by the
/// bootloader.)
pub trait WorkloadSupervisor {
    /// Stop the running workload and start `side` (no Agent reboot).
    fn switch_to(&mut self, side: Side);
    /// Restart `side` after a failed/unhealthy workload.
    fn restore(&mut self, side: Side);
}

/// Why an update step was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The request is for the other target.
    WrongTarget,
    /// A candidate is pending confirmation: nothing may supersede it yet.
    Busy,
    /// Nothing is staged.
    NothingStaged,
    /// The Workload requires a newer runtime API than the Agent provides.
    IncompatibleRuntimeApi,
}

/// Installed Agent artifact: what is known about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledAgent {
    pub artifact: ArtifactDescriptor,
    pub provides: RuntimeApi,
}

/// Installed Workload artifact: what is known about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledWorkload {
    pub artifact: ArtifactDescriptor,
    pub requires: RuntimeApi,
}

/// Outcome of activating an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    /// Agent: the next boot is scheduled, a reboot is required.
    RebootRequired,
    /// Workload: the new workload is running now, no Agent reboot.
    Switched,
}

/// Outcome of the confirmation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    Confirmed,
    /// The candidate was rejected and the previous one restored.
    RolledBack(Reason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Agent self-check / Workload health failed.
    Unhealthy,
    /// A new Agent that cannot run the active Workload is not confirmed.
    IncompatibleWorkload,
}

/// Agent OTA policy: bootloader-driven. Acts only through a [`BootAuthority`].
#[derive(Debug, Clone)]
pub struct AgentOta {
    slots: AbSlots<InstalledAgent>,
    /// The side to restore if the candidate is not confirmed.
    pending_from: Option<Side>,
}

impl AgentOta {
    pub fn new(slots: AbSlots<InstalledAgent>) -> Self {
        Self { slots, pending_from: None }
    }

    pub fn slots(&self) -> &AbSlots<InstalledAgent> {
        &self.slots
    }

    pub fn provided_api(&self) -> Option<RuntimeApi> {
        self.slots.active().map(|a| a.provides)
    }

    /// Writes the order's artifact to the inactive Agent slot.
    pub fn stage(&mut self, request: &UpdateRequest) -> Result<Side, Refusal> {
        let Compatibility::Agent { provides } = request.compatibility() else {
            return Err(Refusal::WrongTarget);
        };
        if self.pending_from.is_some() {
            return Err(Refusal::Busy);
        }
        Ok(self.slots.write_inactive(InstalledAgent { artifact: request.artifact().clone(), provides }))
    }

    /// Schedules the staged Agent for the next boot through the bootloader.
    pub fn activate<B: BootAuthority>(&mut self, boot: &mut B) -> Result<Activation, Refusal> {
        if self.slots.inactive().is_none() {
            return Err(Refusal::NothingStaged);
        }
        let candidate = self.slots.inactive_side();
        self.pending_from = Some(self.slots.active_side());
        boot.schedule_boot(candidate);
        self.slots.select(candidate);
        Ok(Activation::RebootRequired)
    }

    /// After the reboot: confirm or roll back. `self_check_ok` is the Agent's
    /// own check; `active_workload` is what the Agent must still be able to
    /// run -- an Agent that boots but cannot run it is **not** confirmed.
    pub fn confirm<B: BootAuthority>(
        &mut self,
        boot: &mut B,
        self_check_ok: bool,
        active_workload: Option<&InstalledWorkload>,
    ) -> Confirmation {
        let Some(previous) = self.pending_from.take() else {
            return Confirmation::Confirmed;
        };
        let reason = if !self_check_ok {
            Some(Reason::Unhealthy)
        } else if let (Some(api), Some(w)) = (self.provided_api(), active_workload) {
            (!api.satisfies(w.requires)).then_some(Reason::IncompatibleWorkload)
        } else {
            None
        };
        match reason {
            None => Confirmation::Confirmed,
            Some(reason) => {
                boot.restore_boot(previous);
                self.slots.select(previous);
                Confirmation::RolledBack(reason)
            }
        }
    }
}

/// Workload OTA policy: Agent-driven. Acts only through a
/// [`WorkloadSupervisor`]; never touches boot.
#[derive(Debug, Clone)]
pub struct WorkloadOta {
    slots: AbSlots<InstalledWorkload>,
    pending_from: Option<Side>,
}

impl WorkloadOta {
    pub fn new(slots: AbSlots<InstalledWorkload>) -> Self {
        Self { slots, pending_from: None }
    }

    pub fn slots(&self) -> &AbSlots<InstalledWorkload> {
        &self.slots
    }

    pub fn active(&self) -> Option<&InstalledWorkload> {
        self.slots.active()
    }

    /// Writes the order's artifact to the inactive Workload slot.
    pub fn stage(&mut self, request: &UpdateRequest) -> Result<Side, Refusal> {
        let Compatibility::Workload { requires } = request.compatibility() else {
            return Err(Refusal::WrongTarget);
        };
        if self.pending_from.is_some() {
            return Err(Refusal::Busy);
        }
        Ok(self.slots.write_inactive(InstalledWorkload { artifact: request.artifact().clone(), requires }))
    }

    /// Switches to the staged Workload, **after** checking it against the
    /// runtime API the running Agent provides; an incompatible Workload is
    /// refused before it becomes active (the supervisor is never called).
    pub fn activate<S: WorkloadSupervisor>(
        &mut self,
        supervisor: &mut S,
        agent_api: RuntimeApi,
    ) -> Result<Activation, Refusal> {
        let Some(candidate) = self.slots.inactive() else {
            return Err(Refusal::NothingStaged);
        };
        if !agent_api.satisfies(candidate.requires) {
            return Err(Refusal::IncompatibleRuntimeApi);
        }
        let side = self.slots.inactive_side();
        self.pending_from = Some(self.slots.active_side());
        supervisor.switch_to(side);
        self.slots.select(side);
        Ok(Activation::Switched)
    }

    /// After the health window: confirm, or restart the previous Workload.
    pub fn confirm<S: WorkloadSupervisor>(&mut self, supervisor: &mut S, healthy: bool) -> Confirmation {
        let Some(previous) = self.pending_from.take() else {
            return Confirmation::Confirmed;
        };
        if healthy {
            return Confirmation::Confirmed;
        }
        supervisor.restore(previous);
        self.slots.select(previous);
        Confirmation::RolledBack(Reason::Unhealthy)
    }
}

#[cfg(test)]
mod tests;
