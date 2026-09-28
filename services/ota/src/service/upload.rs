//! Streaming upload lifecycle. A platform supplies only physical writes;
//! this manager owns session identity, watermarks and durable publication.

use alloc::string::String;
use core::fmt::Debug;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Instant;

use crate::{Committed, Digest, Error, WriteSession};
use crate::metadata::{MetadataError, MetadataStore, SessionParams, format_digest};
use super::publish;

#[derive(Debug, Clone, Copy, Default)]
pub struct UploadStats {
    pub sectors_flushed: u32,
    pub erase_batches: u32,
    pub erase_batch_kib: usize,
}

#[allow(async_fn_in_trait)]
pub trait UploadWriter {
    type Error: Debug;
    fn slot(&self) -> &'static str;
    fn stats(&self) -> UploadStats { UploadStats::default() }
    async fn append(&mut self, session: &mut WriteSession, data: &[u8]) -> bool;
    async fn finish(&mut self, session: WriteSession) -> Result<Committed, Error<Self::Error>>;
}

struct Upload<W> {
    writer: W,
    engine: WriteSession,
    params: SessionParams,
    started_at: Instant,
}

pub struct UploadManager<W> {
    current: Mutex<CriticalSectionRawMutex, Option<Upload<W>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError { TooLarge, Busy, Conflict, Storage(MetadataError) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishError {
    NotWriting,
    DigestMismatch(Digest),
    Incomplete { durable: u64 },
    Backend,
    Storage(MetadataError),
}

pub struct UploadResult {
    pub written: u32,
    pub digest: String,
    pub slot: &'static str,
    pub elapsed_ms: u64,
    pub stats: UploadStats,
}

impl<W: UploadWriter> UploadManager<W> {
    pub const fn new() -> Self { Self { current: Mutex::new(None) } }

    pub async fn in_progress(&self) -> bool { self.current.lock().await.is_some() }

    pub async fn received(&self) -> u32 {
        self.current.lock().await.as_ref().map_or(0, |s| s.engine.received() as u32)
    }

    pub async fn written(&self) -> u32 {
        self.current.lock().await.as_ref().map_or(0, |s| s.engine.durable() as u32)
    }

    pub async fn params_match(&self, params: &SessionParams) -> bool {
        self.current.lock().await.as_ref().is_some_and(|s| s.params.matches(params))
    }

    /// A new writer is created only after the old staged transaction is
    /// resolved. The caller must select a target without holding flash across
    /// the metadata commit performed here.
    pub async fn begin<S: MetadataStore>(
        &self,
        store: &S,
        params: SessionParams,
        capacity: u64,
        writer: W,
    ) -> Result<(), StartError> {
        if u64::from(params.total) > capacity { return Err(StartError::TooLarge); }
        let expected = super::begin(store, &params).await.map_err(|error| match error {
            super::BeginError::BadDigest => StartError::Busy,
            super::BeginError::Conflict => StartError::Conflict,
            super::BeginError::Storage(error) => StartError::Storage(error),
        })?;
        *self.current.lock().await = Some(Upload {
            writer,
            engine: WriteSession::begin(u64::from(params.total), expected),
            params,
            started_at: Instant::now(),
        });
        Ok(())
    }

    pub async fn chunk(&self, data: &[u8]) -> bool {
        let mut guard = self.current.lock().await;
        let Some(upload) = guard.as_mut() else { return false };
        upload.writer.append(&mut upload.engine, data).await
    }

    pub async fn finish<S: MetadataStore>(&self, store: &S) -> Result<UploadResult, FinishError> {
        let Some(Upload { mut writer, engine, params, started_at }) = self.current.lock().await.take() else {
            return Err(FinishError::NotWriting);
        };
        let slot = writer.slot();
        let committed = writer.finish(engine).await.map_err(|error| match error {
            Error::DigestMismatch(digest) => FinishError::DigestMismatch(digest),
            Error::Incomplete { durable } => FinishError::Incomplete { durable },
            _ => FinishError::Backend,
        })?;
        publish(store, params.deployment_id, committed, String::from(slot))
            .await.map_err(FinishError::Storage)?;
        Ok(UploadResult {
            written: committed.size as u32,
            digest: format_digest(&committed.digest),
            slot,
            elapsed_ms: started_at.elapsed().as_millis(),
            stats: writer.stats(),
        })
    }
}
