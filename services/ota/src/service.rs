//! Durable OTA workflow shared by platform adapters.
//!
//! Each operation publishes metadata before changing a boot slot. A platform
//! must reject invalid target names before the activating record is persisted.

use alloc::string::String;

use crate::{Committed, Error, TransactionState, activate};
use crate::metadata::{
    MetadataError, MetadataStore, MemoryTransactionMetadata, SessionParams,
    can_supersede, commit_transaction, firmware_record, load_transaction,
    parse_digest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginError {
    BadDigest,
    Conflict,
    Storage(MetadataError),
}

/// Resolve the previous staged transaction before writing any new flash bytes.
/// An activating transaction can never be overwritten by an upload.
pub async fn begin<S: MetadataStore>(store: &S, params: &SessionParams) -> Result<crate::Digest, BeginError> {
    let digest = parse_digest(&params.digest).ok_or(BeginError::BadDigest)?;
    match load_transaction(store).await.map_err(BeginError::Storage)? {
        None => {}
        Some(record) if can_supersede(Some(&record)) => {
            commit_transaction(store, None).await.map_err(BeginError::Storage)?;
        }
        Some(_) => return Err(BeginError::Conflict),
    }
    Ok(digest)
}

/// Publish only an artifact whose bytes and digest were already verified.
pub async fn publish<S: MetadataStore>(
    store: &S,
    deployment_id: String,
    artifact: Committed,
    target: String,
) -> Result<(), MetadataError> {
    let record = firmware_record(deployment_id, artifact.size, artifact.digest, target);
    commit_transaction(store, Some(&record)).await
}

/// Validate the target without touching boot state, then activate it.
#[allow(async_fn_in_trait)]
pub trait BootActivation {
    fn valid_target(&self, target: &str) -> bool;
    async fn activate(&self, target: &str) -> Result<(), ()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateError {
    NotStaged,
    DeploymentMismatch,
    Storage(MetadataError),
}

/// Publish `Activating` before programming the boot entry. If the boot entry
/// fails, restore `Staged` so activation can be retried. A failed restore is
/// still recoverable: the next boot reconciles the persisted state.
pub async fn activate_staged<S: MetadataStore, B: BootActivation>(
    store: &S,
    boot: &B,
    deployment_id: &str,
) -> Result<String, ActivateError> {
    let current = load_transaction(store).await.map_err(ActivateError::Storage)?;
    let target = current.as_ref()
        .and_then(|record| record.artifacts.first())
        .map(|artifact| artifact.target.clone())
        .filter(|target| boot.valid_target(target))
        .ok_or(ActivateError::NotStaged)?;
    let mut memory = MemoryTransactionMetadata { record: current };
    let activating = activate(&mut memory, &String::from(deployment_id)).map_err(|error| match error {
        Error::IdentityMismatch => ActivateError::DeploymentMismatch,
        _ => ActivateError::NotStaged,
    })?;
    commit_transaction(store, memory.record.as_ref()).await.map_err(ActivateError::Storage)?;
    if boot.activate(&target).await.is_err() {
        let reverted = activating.with_state(TransactionState::Staged);
        let _ = commit_transaction(store, Some(&reverted)).await;
        return Err(ActivateError::NotStaged);
    }
    Ok(target)
}
