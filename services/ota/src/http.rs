//! Streaming `PUT /ota/write`: a route that can't use
//! picoserve's usual `String`/`Form` body extractors -- those buffer the
//! *entire* body before a handler ever runs, and an OTA image (hundreds of
//! KB) doesn't fit this device's heap. Exposes a streaming handler for the
//! shared `iobewi-http` router, writing one `read()` at a time.

use alloc::format;
use alloc::string::String;

use iobewi_http::io::Read;
use iobewi_http::request::Request;
use iobewi_http::auth::{bearer_token, Bearer};
use iobewi_http::json::{json_error, json_ok, JsonResponse};
use iobewi_http::range::parse_content_range;
use iobewi_http::response::{IntoResponse, ResponseWriter, StatusCode};
use iobewi_http::routing::{post, put_service, PathRouter, RequestHandlerService};
use iobewi_http::stream::{ChunkSink, StreamError, stream_exact};
use iobewi_http::{HttpRouter, ResponseSent};
use serde::{Deserialize, Serialize};

use crate::metadata::{PrepareRefusal, SessionParams};
use crate::{ResumePlan, is_complete, resume_plan};

fn unauthorized() -> JsonResponse {
    json_error(StatusCode::UNAUTHORIZED, "{\"error\":\"unauthorized\"}")
}

/// Platform and application effects needed by the streaming OTA route.
/// The authentication policy and flash writer are supplied by the caller.
#[allow(async_fn_in_trait)]
pub trait WriteBackend {
    async fn authorize(&self, token: &str) -> bool;
    async fn in_progress(&self) -> bool;
    async fn received(&self) -> u32;
    async fn written(&self) -> u32;
    async fn params_match(&self, params: &SessionParams) -> bool;
    async fn begin(&self, params: SessionParams) -> Result<(), BeginError>;
    async fn chunk(&self, bytes: &[u8]) -> bool;
    async fn finish(&self) -> Result<WriteFinishOk, WriteFinishError>;
}

#[derive(Debug)]
pub enum BeginError { Busy, TooLarge, Conflict, Storage }

pub struct WriteFinishOk { pub written: u32, pub digest: String }

#[derive(Debug)]
pub enum WriteFinishError { NotWriting, DigestMismatch, Incomplete, Storage }

#[derive(Deserialize)]
pub struct PrepareRequest {
    pub size: u32,
    pub chip: String,
    pub partition_layout: String,
}

#[derive(Serialize)]
pub struct PrepareResponse {
    pub accepted: bool,
    pub target_slot: Option<&'static str>,
    pub reason: Option<&'static str>,
}

impl PrepareResponse {
    pub fn refuse(reason: PrepareRefusal) -> Self {
        Self { accepted: false, target_slot: None, reason: Some(reason.reason()) }
    }

    pub fn accept(target_slot: &'static str) -> Self {
        Self { accepted: true, target_slot: Some(target_slot), reason: None }
    }
}

#[derive(Deserialize)]
pub struct ActivateRequest {
    pub deployment_id: String,
}

#[allow(async_fn_in_trait)]
pub trait ControlBackend {
    async fn prepare(&self, request: &PrepareRequest) -> PrepareResponse;
    async fn activate(&self, deployment_id: &str) -> Result<String, ActivateFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateFailure { NotStaged, DeploymentMismatch, Storage }

pub async fn prepare_response<B: ControlBackend>(backend: &B, body: &str) -> JsonResponse {
    let Ok(request) = serde_json::from_str::<PrepareRequest>(body) else {
        return json_error(StatusCode::BAD_REQUEST, "{\"error\":\"bad_request\"}");
    };
    json_ok(serde_json::to_string(&backend.prepare(&request).await).unwrap_or_default())
}

/// The caller must schedule its platform reboot after this returns `true`.
/// This keeps the one-shot reset peripheral out of portable route code.
pub async fn activate_response<B: ControlBackend>(backend: &B, body: &str) -> (JsonResponse, bool) {
    let Ok(request) = serde_json::from_str::<ActivateRequest>(body) else {
        return (json_error(StatusCode::BAD_REQUEST, "{\"error\":\"missing_deployment_id\"}"), false);
    };
    match backend.activate(&request.deployment_id).await {
        Ok(slot) => (json_ok(format!("{{\"status\":\"rebooting\",\"target_slot\":\"{slot}\"}}")), true),
        Err(ActivateFailure::DeploymentMismatch) =>
            (json_error(StatusCode::CONFLICT, "{\"error\":\"deployment_mismatch\"}"), false),
        Err(ActivateFailure::NotStaged) =>
            (json_error(StatusCode::CONFLICT, "{\"error\":\"not_staged\"}"), false),
        Err(ActivateFailure::Storage) =>
            (json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"error\":\"nvs_write_failed\"}"), false),
    }
}

/// Platform action requested only after an OTA activation succeeded. The
/// implementation must defer reset until the HTTP response can be sent.
#[allow(async_fn_in_trait)]
pub trait RebootPort {
    async fn schedule_reboot(&self);
}

/// The complete OTA router, relative to the caller's API namespace.
/// Authorization, flash/NVS access and reboot remain injectable capabilities.
pub fn routes<B, R>(backend: B, reboot: R) -> HttpRouter<impl PathRouter>
where
    B: ControlBackend + WriteBackend + Clone,
    R: RebootPort + Clone,
{
    let prepare_backend = backend.clone();
    let write_backend = backend.clone();
    HttpRouter::new()
        .route("/prepare", post(move |Bearer(token): Bearer, body: String| {
            let backend = prepare_backend.clone();
            async move {
                if !backend.authorize(token.as_deref().unwrap_or("")).await {
                    return unauthorized();
                }
                prepare_response(&backend, &body).await
            }
        }))
        .route("/write", put_service(OtaWrite { backend: write_backend }))
        .route("/activate", post(move |Bearer(token): Bearer, body: String| {
            let backend = backend.clone();
            let reboot = reboot.clone();
            async move {
                if !backend.authorize(token.as_deref().unwrap_or("")).await {
                    return unauthorized();
                }
                let (response, should_reboot) = activate_response(&backend, &body).await;
                if should_reboot {
                    reboot.schedule_reboot().await;
                }
                response
            }
        }))
}

/// Wire-format validation for X-Embewi-Digest.
fn is_valid_digest(value: &str) -> bool {
    crate::metadata::parse_digest(value).is_some()
}

/// The OTA metadata format limits offsets and firmware size to `u32`.
fn ota_range(value: &str) -> Option<(u32, u32, u32, u64)> {
    let range = parse_content_range(value)?;
    Some((
        u32::try_from(range.start).ok()?,
        u32::try_from(range.end).ok()?,
        u32::try_from(range.total).ok()?,
        range.len()?,
    ))
}

pub struct OtaWrite<B> {
    pub backend: B,
}

impl<B: WriteBackend> ChunkSink for OtaWrite<B> {
    type Error = ();

    async fn write_chunk(&self, chunk: &[u8]) -> Result<(), Self::Error> {
        self.backend.chunk(chunk).await.then_some(()).ok_or(())
    }
}

impl<B: WriteBackend> RequestHandlerService for OtaWrite<B> {
    async fn call_request_handler_service<R: Read, W: ResponseWriter<Error = R::Error>>(
        &self,
        _state: &(),
        _path_parameters: (),
        mut request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let headers = request.parts.headers();
        let token = bearer_token(headers
            .get("authorization")
            .and_then(|v| v.as_str().ok()));
        if !self.backend.authorize(token).await {
            return unauthorized()
                .write_to(request.body_connection.finalize().await?, response_writer)
                .await;
        }

        let deployment_id = headers.get("x-embewi-deployment-id").and_then(|v| v.as_str().ok()).unwrap_or("");
        let digest = headers.get("x-embewi-digest").and_then(|v| v.as_str().ok()).unwrap_or("");
        let content_range = headers.get("content-range").and_then(|v| v.as_str().ok());
        let content_length = request.body_connection.content_length();

        // Refuse before touching the session: an invalid PUT must not
        // disturb one in progress. What a session is (deployment, digest,
        // total) is fixed by its first PUT and must be repeated verbatim.
        let bad_request = |error: &'static str| json_error(StatusCode::BAD_REQUEST, error);
        let invalid = if deployment_id.is_empty() {
            Some("{\"error\":\"missing_deployment_id\"}")
        } else if !is_valid_digest(digest) {
            Some("{\"error\":\"bad_digest\"}")
        } else {
            None
        };
        if let Some(error) = invalid {
            return bad_request(error)
                .write_to(request.body_connection.finalize().await?, response_writer)
                .await;
        }

        let (has_range, start, end, total) = match content_range {
            // Monolithic PUT: the body is the whole image.
            None => match u32::try_from(content_length).ok().filter(|len| *len > 0) {
                Some(len) => (false, 0u32, len - 1, len),
                None => {
                    return bad_request("{\"error\":\"empty_body\"}")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
            },
            Some(value) => match ota_range(value) {
                Some((s, e, t, len)) if len == content_length as u64 => {
                    (true, s, e, t)
                }
                Some(_) => {
                    return bad_request("{\"error\":\"content_length_mismatch\"}")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
                None => {
                    return bad_request("{\"error\":\"bad_content_range\"}")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
            },
        };
        let params = SessionParams {
            deployment_id: String::from(deployment_id),
            digest: String::from(digest),
            total,
        };

        let in_progress = self.backend.in_progress().await;
        // The Continue-vs-Resync decision: how much this session has
        // *accepted* so far (flushed to flash or still buffered), which is
        // what an uninterrupted client's next chunk continues from --
        // distinct from `ota::write_written` (flushed only), reported to
        // the client below as the durable point to resume from after a
        // dropped connection.
        let received_so_far = self.backend.received().await;
        match resume_plan(has_range, u64::from(start), in_progress, u64::from(received_so_far)) {
            ResumePlan::Begin => match self.backend.begin(params).await {
                Ok(()) => {}
                Err(BeginError::TooLarge) => {
                    return json_error(StatusCode::PAYLOAD_TOO_LARGE, "{\"error\":\"size_too_large\"}")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
                Err(BeginError::Busy) => {
                    return json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"status\":\"ota_begin_failed\"}")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
                Err(BeginError::Conflict) => {
                    return json_error(StatusCode::CONFLICT, "{\"error\":\"ota_busy\"}")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
                Err(BeginError::Storage) => {
                    return json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"status\":\"nvs_write_failed\"}")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
            },
            ResumePlan::Resync => {
                let written = self.backend.written().await;
                return json_error(
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    &format!("{{\"error\":\"range_mismatch\",\"written\":{written}}}"),
                )
                .write_to(request.body_connection.finalize().await?, response_writer)
                .await;
            }
            ResumePlan::Continue => {
                if !self.backend.params_match(&params).await {
                    return json_error(StatusCode::CONFLICT, "{\"error\":\"session_mismatch\"}")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
            }
        }

        let mut buf = [0u8; 1024];
        let outcome = {
            let mut reader = request.body_connection.body().reader();
            stream_exact(&mut reader, content_length, &mut buf, self).await
        };

        match outcome {
            Ok(()) => {}
            Err(StreamError::Read(error)) => return Err(error),
            Err(StreamError::UnexpectedEof | StreamError::Write(()) | StreamError::EmptyBuffer) => {
                return json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"status\":\"write_failed\"}")
                    .write_to(request.body_connection.finalize().await?, response_writer)
                    .await;
            }
        }

        if !is_complete(has_range, u64::from(end), u64::from(total)) {
            let written = self.backend.written().await;
            return json_ok(format!("{{\"status\":\"partial\",\"written\":{written}}}"))
                .write_to(request.body_connection.finalize().await?, response_writer)
                .await;
        }

        let response: JsonResponse = match self.backend.finish().await {
            Ok(result) => json_ok(format!(
                "{{\"written\":{},\"digest\":\"{}\",\"status\":\"written\"}}",
                result.written, result.digest
            )),
            Err(WriteFinishError::DigestMismatch) => json_ok(String::from("{\"status\":\"digest_mismatch\"}")),
            Err(WriteFinishError::Storage) => {
                json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"status\":\"nvs_write_failed\"}")
            }
            Err(WriteFinishError::NotWriting | WriteFinishError::Incomplete) => {
                json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"status\":\"write_failed\"}")
            }
        };
        response
            .write_to(request.body_connection.finalize().await?, response_writer)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ota_range_rejects_images_exceeding_metadata_limits() {
        assert_eq!(ota_range("bytes 1024-2047/4096"), Some((1024, 2047, 4096, 1024)));
        assert_eq!(ota_range("bytes 0-4294967295/4294967296"), None);
    }
}
