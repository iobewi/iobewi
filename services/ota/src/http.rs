//! Streaming `PUT /v1alpha1/ota/write`: a route that can't use
//! picoserve's usual `String`/`Form` body extractors -- those buffer the
//! *entire* body before a handler ever runs, and an OTA image (hundreds of
//! KB) doesn't fit this device's heap. Implements `RequestHandlerService`
//! directly instead, streaming the body straight into flash one `read()`
//! at a time.

use alloc::format;
use alloc::string::String;

use picoserve::io::Read;
use picoserve::request::Request;
use picoserve::response::{ContentBody, ContentHeaders, IntoResponse, Response, ResponseWriter, StatusCode};
use picoserve::routing::RequestHandlerService;
use picoserve::ResponseSent;
use serde::{Deserialize, Serialize};

use crate::metadata::{PrepareRefusal, SessionParams};
use crate::{ResumePlan, is_complete, resume_plan};

pub type JsonResponse = Response<ContentHeaders, ContentBody<String>>;

fn json_ok(body: String) -> JsonResponse {
    Response::ok(body).with_content_type("application/json")
}

fn json_error(status: StatusCode, body: &str) -> JsonResponse {
    Response::new(status, String::from(body)).with_content_type("application/json")
}

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

/// Number of bytes carried by an inclusive HTTP Content-Range.
fn range_len(start: u32, end: u32) -> Option<u32> {
    end.checked_sub(start)?.checked_add(1)
}

/// Wire-format validation for X-Embewi-Digest.
fn is_valid_digest(value: &str) -> bool {
    crate::metadata::parse_digest(value).is_some()
}

/// Parses Content-Range: bytes <start>-<end>/<total>.
///
/// This is deliberately HTTP-local. Resume/session decisions themselves
/// remain in IOBEWI OTA; only the wire syntax belongs to this route.
fn parse_content_range(value: &str) -> Option<(u32, u32, u32)> {
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start: u32 = start.trim().parse().ok()?;
    let end: u32 = end.trim().parse().ok()?;
    let total: u32 = total.trim().parse().ok()?;
    (start <= end && end < total).then_some((start, end, total))
}

pub struct OtaWrite<B> {
    pub backend: B,
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
        let token = headers
            .get("authorization")
            .and_then(|v| v.as_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
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
            Some(value) => match parse_content_range(value) {
                Some((s, e, t)) if range_len(s, e).is_some_and(|len| len as usize == content_length) => {
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
        let mut remaining = content_length;
        let mut chunk_error = false;
        {
            let mut reader = request.body_connection.body().reader();
            while remaining > 0 {
                let to_read = remaining.min(buf.len());
                let n = reader.read(&mut buf[..to_read]).await?;
                if n == 0 {
                    chunk_error = true;
                    break;
                }
                if !self.backend.chunk(&buf[..n]).await {
                    chunk_error = true;
                    break;
                }
                remaining -= n;
            }
        }

        if chunk_error {
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"status\":\"write_failed\"}")
                .write_to(request.body_connection.finalize().await?, response_writer)
                .await;
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
    fn range_header_rejects_overflow_and_bad_bounds() {
        assert_eq!(parse_content_range("bytes 1024-2047/4096"), Some((1024, 2047, 4096)));
        assert_eq!(parse_content_range("bytes 2048-1024/4096"), None);
        assert_eq!(parse_content_range("bytes 0-4096/4096"), None);
        assert_eq!(parse_content_range("bytes 0-4294967296/4294967297"), None);
        assert_eq!(range_len(0, u32::MAX), None);
    }
}
