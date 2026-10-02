//! Boot reconciliation and confirmation gate independent of any SoC.

use alloc::string::String;

use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};

use crate::{Action, BackendOutcome};
use crate::metadata::{
    Metadata, MetadataStore, PendingCheckAction, boot_action, clear_staged,
    finish_validation, load_metadata, pending_check_action,
};

/// What the platform can tell and do about the boot chain. `BootOps` has
/// always bundled three distinct capabilities; they are named separately so
/// each can be classified and implemented where it belongs, and the bundle
/// stays a blanket implementation (the service functions keep one bound):
///
/// * [`BootState`] -- physical boot status and the durable EWBT writes
///   (`firmware/boot` logic executed by a platform storage adapter);
/// * [`BootSelfCheck`] -- the storage self-check at boot;
/// * [`BootWatchdog`] -- the pending-verify watchdog (primitive in a
///   watchdog driver; the service decides when to feed or disable it).
///
/// The service decides when to use them. The caller decides when its own
/// application is ready to validate.
#[allow(async_fn_in_trait)]
pub trait BootState {
    async fn image_outcome(&self) -> BackendOutcome;
    async fn booted_slot(&self) -> String;
    async fn confirm(&self) -> bool;
    async fn reject(&self) -> bool;
}

#[allow(async_fn_in_trait)]
pub trait BootSelfCheck {
    async fn self_check(&self) -> bool;
}

pub trait BootWatchdog {
    fn watchdog_feed(&self);
    fn watchdog_disable(&self);
}

/// Everything the boot gate needs from a platform: the three capabilities above.
pub trait BootOps: BootState + BootSelfCheck + BootWatchdog {}

impl<T: BootState + BootSelfCheck + BootWatchdog> BootOps for T {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStatus { Stable, PendingVerify, Degraded, Rollback }

pub struct BootResult {
    pub status: BootStatus,
    pub image: BackendOutcome,
    pub slot: String,
    pub action: Action,
    pub storage_healthy: Option<bool>,
}

/// Reconcile the durable transaction with the image actually booted. A
/// rollback request rejects the candidate before asking the caller to reset.
pub async fn on_boot<S: MetadataStore, B: BootOps>(store: &S, boot: &B) -> BootResult {
    let staged = load_metadata(store).await.unwrap_or_else(|_| Metadata::default()).staged;
    let image = boot.image_outcome().await;
    let slot = boot.booted_slot().await;
    let action = boot_action(&staged, image, (!slot.is_empty()).then_some(slot.as_str()));
    let status = match action {
        Action::AwaitConfirmation => {
            boot.watchdog_feed();
            BootStatus::PendingVerify
        }
        Action::RollbackUnaccounted => {
            let _ = boot.reject().await;
            BootStatus::Rollback
        }
        Action::ClearStale => {
            let _ = clear_staged(store).await;
            boot.watchdog_disable();
            BootStatus::Stable
        }
        Action::FinishInterruptedActivation => {
            let outcome = finish_validation(store, &staged).await;
            boot.watchdog_disable();
            if outcome.is_err() { BootStatus::Degraded } else { BootStatus::Stable }
        }
        _ => {
            boot.watchdog_disable();
            BootStatus::Stable
        }
    };
    // A failed metadata commit after confirmation is retried on the next
    // boot. Do not claim full health or run another self-check in that case.
    let storage_healthy = if status == BootStatus::Stable {
        Some(boot.self_check().await)
    } else { None };
    BootResult { status, image, slot, action, storage_healthy }
}

/// Race the storage check against a bounded deadline. A timeout leaves the
/// boot entry Pending so that a reset causes the bootloader to roll back.
pub async fn pending_check<B: BootOps>(boot: &B, timeout: Duration) -> PendingCheckAction {
    let result = match select(boot.self_check(), Timer::after(timeout)).await {
        Either::First(passed) => Some(passed),
        Either::Second(()) => None,
    };
    pending_check_action(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation { Valid, Degraded, ResetRequired }

/// Call only after `pending_check` returned `Confirm`. The watchdog remains
/// armed until the boot entry has been durably confirmed by the adapter.
pub async fn confirm_pending<S: MetadataStore, B: BootOps>(store: &S, boot: &B) -> Confirmation {
    let staged = load_metadata(store).await.unwrap_or_else(|_| Metadata::default()).staged;
    if !boot.confirm().await {
        let _ = boot.reject().await;
        return Confirmation::ResetRequired;
    }
    boot.watchdog_disable();
    if finish_validation(store, &staged).await.is_err() {
        Confirmation::Degraded
    } else {
        Confirmation::Valid
    }
}

pub async fn reject_pending<B: BootOps>(boot: &B) {
    let _ = boot.reject().await;
}
