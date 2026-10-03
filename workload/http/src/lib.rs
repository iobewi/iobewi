#![no_std]

//! HTTP surface of the Workload OTA, mounted by the application under its own
//! namespace (`<api>/workload/ota/...`), separately from the Agent's `/ota/*`
//! routes (which are untouched and keep meaning `UpdateTarget::Agent`):
//!
//! | method | path | purpose |
//! |---|---|---|
//! | GET  | `/status`   | capability and OTM2 state (200 even when unsupported) |
//! | POST | `/prepare`  | name the artifact: id, version, size, SHA-256, required runtime API |
//! | PUT  | `/write`    | streaming `Content-Range` upload, SHA-256 verified, stages on completion |
//! | POST | `/activate` | `Staged -> Activating -> PendingConfirmation` through the Supervisor (501 without one) |
//! | POST | `/confirm`  | `PendingConfirmation -> Valid`, only if the candidate runs and is healthy |
//! | POST | `/rollback` | `PendingConfirmation -> RollingBack -> previous Valid / Empty` |
//!
//! This crate is HTTP only: parsing, auth integration, request -> service
//! mapping, error -> status mapping. The state machine, OTM2 and the streaming
//! engine are `iobewi-workload-ota`; the physical backend (flash, partitions) is
//! injected through the service. No ESP, flash, reset or reboot type appears
//! here, and no Workload route ever reboots the Agent.

extern crate alloc;
#[cfg(test)]
extern crate std;

use alloc::format;
use alloc::string::String;

use iobewi_http_server::auth::{Bearer, bearer_token};
use iobewi_http_server::io::Read;
use iobewi_http_server::json::{JsonResponse, json_error, json_ok};
use iobewi_http_server::range::parse_content_range;
use iobewi_http_server::request::Request;
use iobewi_http_server::response::{IntoResponse, ResponseWriter, StatusCode};
use iobewi_http_server::routing::{PathRouter, RequestHandlerService, get, post, put_service};
use iobewi_http_server::stream::{ChunkSink, StreamError, stream_exact};
use iobewi_http_server::{HttpRouter, ResponseSent};
use iobewi_ota::metadata::format_digest;
use iobewi_ota::{ResumePlan, is_complete, resume_plan};
use iobewi_update_model::RuntimeApi;
use iobewi_workload_ota::flash::FlashAccess;
use iobewi_workload_ota::otm2::State;
use iobewi_workload_ota::service::{ArtifactInfo, PrepareInput, ServiceError, Status, WorkloadOtaService};
use iobewi_workload_ota::supervisor::{RuntimeStatus, WorkloadRuntime, WorkloadSupervisor};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Bearer policy, supplied by the application (the same admin token as the Agent OTA).
#[allow(async_fn_in_trait)]
pub trait Authorize {
    async fn authorize(&self, token: &str) -> bool;
}

/// The control authority behind activate/confirm/rollback. `NoSupervisor` is the
/// production default of a build without a runtime: every check runs, then the
/// operation is refused (501) and the state is never touched. A build with a real
/// Supervisor plugs it in through [`WorkloadSupervisor`] (see the impl below).
#[allow(async_fn_in_trait)]
pub trait ControlPort<A: FlashAccess> {
    async fn activate(&self, service: &WorkloadOtaService<A>, digest: &[u8; 32]) -> Result<(), ServiceError>;
    async fn confirm(&self, service: &WorkloadOtaService<A>) -> Result<(), ServiceError>;
    async fn rollback(&self, service: &WorkloadOtaService<A>) -> Result<(), ServiceError>;
    /// Execution status, `None` when no supervisor exists.
    async fn runtime_status(&self) -> Option<RuntimeStatus>;
}

#[derive(Clone, Copy, Default)]
pub struct NoSupervisor;

impl<A: FlashAccess> ControlPort<A> for NoSupervisor {
    async fn activate(&self, service: &WorkloadOtaService<A>, digest: &[u8; 32]) -> Result<(), ServiceError> {
        service.check_activation(Some(digest)).await?;
        Err(ServiceError::SupervisorUnavailable)
    }

    async fn confirm(&self, _service: &WorkloadOtaService<A>) -> Result<(), ServiceError> {
        Err(ServiceError::SupervisorUnavailable)
    }

    async fn rollback(&self, _service: &WorkloadOtaService<A>) -> Result<(), ServiceError> {
        Err(ServiceError::SupervisorUnavailable)
    }

    async fn runtime_status(&self) -> Option<RuntimeStatus> {
        None
    }
}

/// A real Supervisor as the control authority.
impl<A: FlashAccess + 'static, R: WorkloadRuntime> ControlPort<A> for &'static WorkloadSupervisor<A, R> {
    async fn activate(&self, _service: &WorkloadOtaService<A>, digest: &[u8; 32]) -> Result<(), ServiceError> {
        WorkloadSupervisor::activate(self, digest).await
    }

    async fn confirm(&self, _service: &WorkloadOtaService<A>) -> Result<(), ServiceError> {
        WorkloadSupervisor::confirm(self).await
    }

    async fn rollback(&self, _service: &WorkloadOtaService<A>) -> Result<(), ServiceError> {
        WorkloadSupervisor::rollback(self).await
    }

    async fn runtime_status(&self) -> Option<RuntimeStatus> {
        Some(WorkloadSupervisor::runtime_status(self).await)
    }
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ApiBody {
    major: u16,
    minor: u16,
}

#[derive(Deserialize)]
struct PrepareBody {
    artifact_id: String,
    version: String,
    size: u64,
    digest: String,
    required_runtime_api: ApiBody,
}

#[derive(Deserialize)]
struct ActivateBody {
    digest: String,
}

#[derive(Serialize)]
struct ApiOut {
    major: u16,
    minor: u16,
}

#[derive(Serialize)]
struct ArtifactOut {
    id: String,
    version: String,
    digest: String,
    size: u32,
    required_runtime_api: ApiOut,
}

#[derive(Serialize)]
struct Diagnostic {
    active_slot: Option<&'static str>,
    candidate_slot: Option<&'static str>,
}

#[derive(Serialize)]
struct RunningOut {
    id: String,
    version: String,
    digest: String,
}

#[derive(Serialize)]
struct RuntimeOut {
    /// Is there a Supervisor at all on this build?
    supervised: bool,
    running: bool,
    health: &'static str,
    artifact: Option<RunningOut>,
}

#[derive(Serialize)]
struct StatusOut {
    supported: bool,
    reason: Option<String>,
    state: &'static str,
    active: Option<ArtifactOut>,
    candidate: Option<ArtifactOut>,
    previous: Option<ArtifactOut>,
    max_artifact_size: u32,
    runtime_api_provided: ApiOut,
    write_in_progress: bool,
    prepared: bool,
    /// Execution state, separate from OTM2's persistent state.
    runtime: RuntimeOut,
    /// Diagnostic only: Core never needs a slot to order an update.
    diagnostic: Diagnostic,
}

fn api_out(api: RuntimeApi) -> ApiOut {
    ApiOut { major: api.major, minor: api.minor }
}

fn artifact_out(info: ArtifactInfo) -> ArtifactOut {
    ArtifactOut {
        id: info.id,
        version: info.version,
        digest: format_digest(&iobewi_ota::Digest(info.digest)),
        size: info.size,
        required_runtime_api: api_out(info.requires),
    }
}

pub fn state_name(state: Option<State>) -> &'static str {
    match state {
        None => "none",
        Some(State::Empty) => "empty",
        Some(State::Valid) => "valid",
        Some(State::Staged) => "staged",
        Some(State::Activating) => "activating",
        Some(State::PendingConfirmation) => "pending_confirmation",
        Some(State::RollingBack) => "rolling_back",
    }
}

fn slot_name(side: Option<iobewi_update_model::Side>) -> Option<&'static str> {
    side.map(|s| match s {
        iobewi_update_model::Side::A => "A",
        iobewi_update_model::Side::B => "B",
    })
}

pub fn status_json(status: Status, runtime: Option<RuntimeStatus>) -> String {
    let runtime = match runtime {
        None => RuntimeOut { supervised: false, running: false, health: "unknown", artifact: None },
        Some(r) => RuntimeOut {
            supervised: true,
            running: r.running.is_some(),
            health: r.health.as_str(),
            artifact: r.running.map(|i| RunningOut {
                id: i.id,
                version: i.version,
                digest: format_digest(&iobewi_ota::Digest(i.digest)),
            }),
        },
    };
    let out = StatusOut {
        supported: status.supported,
        reason: status.reason,
        state: if status.corrupted { "corrupted" } else { state_name(status.state) },
        active: status.active.map(artifact_out),
        candidate: status.candidate.map(artifact_out),
        previous: status.previous.map(artifact_out),
        max_artifact_size: status.max_artifact_size,
        runtime_api_provided: api_out(status.provided),
        write_in_progress: status.write_in_progress,
        prepared: status.prepared,
        runtime,
        diagnostic: Diagnostic {
            active_slot: slot_name(status.active_slot),
            candidate_slot: slot_name(status.candidate_slot),
        },
    };
    serde_json::to_string(&out).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Error mapping (one table; see docs/workload-ota-http.md)
// ---------------------------------------------------------------------------

fn unauthorized() -> JsonResponse {
    json_error(StatusCode::UNAUTHORIZED, "{\"error\":\"unauthorized\"}")
}

fn bad_request(error: &str) -> JsonResponse {
    json_error(StatusCode::BAD_REQUEST, &format!("{{\"error\":\"{error}\"}}"))
}

/// `ServiceError` -> HTTP. Reasons the Agent OTA already expresses reuse its
/// codes (413 size, 416 range, 409 busy/conflict/not staged, 400 malformed,
/// 500 storage); a wrong digest is a real error here (422), unlike the Agent
/// route's historical `200 {"status":"digest_mismatch"}` which is not touched.
pub fn error_response(error: &ServiceError) -> JsonResponse {
    use ServiceError::*;
    match error {
        Unsupported(why) => json_error(
            StatusCode::CONFLICT,
            &format!("{{\"error\":\"workload_storage_unsupported\",\"reason\":\"{why:?}\"}}"),
        ),
        TableUnreadable => json_error(
            StatusCode::CONFLICT,
            "{\"error\":\"workload_storage_unsupported\",\"reason\":\"TableUnreadable\"}",
        ),
        Corrupted => json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"error\":\"otm2_corrupted\"}"),
        Busy(state) => json_error(
            StatusCode::CONFLICT,
            &format!("{{\"error\":\"workload_busy\",\"state\":\"{}\"}}", state_name(Some(*state))),
        ),
        WrongState(state) => json_error(
            StatusCode::CONFLICT,
            &format!("{{\"error\":\"not_staged\",\"state\":\"{}\"}}", state_name(*state)),
        ),
        NotPrepared => json_error(StatusCode::CONFLICT, "{\"error\":\"not_prepared\"}"),
        SessionMismatch => json_error(StatusCode::CONFLICT, "{\"error\":\"session_mismatch\"}"),
        CandidateMismatch => json_error(StatusCode::CONFLICT, "{\"error\":\"candidate_mismatch\"}"),
        TooLarge { max } => json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            &format!("{{\"error\":\"size_too_large\",\"max\":{max}}}"),
        ),
        EmptyArtifact => bad_request("empty_artifact"),
        BadField(_) => bad_request("bad_field"),
        IncompatibleRuntimeApi { required, provided } => json_error(
            StatusCode::CONFLICT,
            &format!(
                "{{\"error\":\"incompatible_runtime_api\",\"required\":\"{}.{}\",\"provided\":\"{}.{}\"}}",
                required.major, required.minor, provided.major, provided.minor
            ),
        ),
        DigestMismatch(computed) => json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("{{\"error\":\"digest_mismatch\",\"computed\":\"{}\"}}", format_digest(&iobewi_ota::Digest(*computed))),
        ),
        Incomplete { durable } => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("{{\"error\":\"incomplete\",\"written\":{durable}}}"),
        ),
        SupervisorUnavailable => json_error(
            StatusCode::NOT_IMPLEMENTED,
            "{\"error\":\"supervisor_unavailable\"}",
        ),
        ImageRejected(reason) => json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("{{\"error\":\"image_rejected\",\"reason\":\"{reason}\"}}"),
        ),
        TransitionInProgress => json_error(StatusCode::CONFLICT, "{\"error\":\"transition_in_progress\"}"),
        ActivationFailed => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "{\"error\":\"activation_failed\",\"rolled_back\":true}",
        ),
        RollbackFailed => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "{\"error\":\"rollback_failed\",\"state\":\"rolling_back\"}",
        ),
        CandidateCorrupted => json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "{\"error\":\"candidate_corrupted\",\"discarded\":true}",
        ),
        NotRunning => json_error(StatusCode::CONFLICT, "{\"error\":\"workload_not_running\"}"),
        Unhealthy(health) => json_error(
            StatusCode::CONFLICT,
            &format!("{{\"error\":\"workload_unhealthy\",\"health\":\"{}\"}}", health.as_str()),
        ),
        Storage => json_error(StatusCode::INTERNAL_SERVER_ERROR, "{\"error\":\"storage_failure\"}"),
    }
}

// ---------------------------------------------------------------------------
// JSON endpoints (plain functions, directly testable)
// ---------------------------------------------------------------------------

fn parse_digest(value: &str) -> Option<[u8; 32]> {
    iobewi_ota::metadata::parse_digest(value).map(|d| d.0)
}

pub async fn prepare_response<A: FlashAccess>(service: &WorkloadOtaService<A>, body: &str) -> JsonResponse {
    let Ok(request) = serde_json::from_str::<PrepareBody>(body) else {
        return bad_request("bad_request");
    };
    let Some(digest) = parse_digest(&request.digest) else {
        return bad_request("bad_digest");
    };
    let input = PrepareInput {
        artifact_id: request.artifact_id,
        version: request.version,
        size: request.size,
        digest,
        requires: RuntimeApi::new(request.required_runtime_api.major, request.required_runtime_api.minor),
    };
    match service.prepare(&input).await {
        Ok(()) => {
            log::info!("workload ota: prepare id={} version={} size={}", input.artifact_id, input.version, input.size);
            let max = service.storage().map_or(0, |s| s.layout().max_artifact_size());
            json_ok(format!("{{\"accepted\":true,\"max_artifact_size\":{max}}}"))
        }
        Err(error) => error_response(&error),
    }
}

pub async fn activate_response<A: FlashAccess, P: ControlPort<A>>(
    service: &WorkloadOtaService<A>,
    port: &P,
    body: &str,
) -> JsonResponse {
    let Ok(request) = serde_json::from_str::<ActivateBody>(body) else {
        return bad_request("missing_digest");
    };
    let Some(digest) = parse_digest(&request.digest) else {
        return bad_request("bad_digest");
    };
    match port.activate(service, &digest).await {
        Ok(()) => json_ok(String::from("{\"status\":\"pending_confirmation\"}")),
        Err(error) => {
            if error == ServiceError::SupervisorUnavailable {
                log::warn!("workload ota: activate refused: supervisor unavailable");
            }
            error_response(&error)
        }
    }
}

pub async fn confirm_response<A: FlashAccess, P: ControlPort<A>>(service: &WorkloadOtaService<A>, port: &P) -> JsonResponse {
    match port.confirm(service).await {
        Ok(()) => {
            log::info!("workload ota: confirmed");
            json_ok(String::from("{\"status\":\"valid\"}"))
        }
        Err(error) => error_response(&error),
    }
}

pub async fn rollback_response<A: FlashAccess, P: ControlPort<A>>(service: &WorkloadOtaService<A>, port: &P) -> JsonResponse {
    match port.rollback(service).await {
        Ok(()) => {
            let state = state_name(service.status().await.state);
            log::info!("workload ota: rolled back, state={state}");
            json_ok(format!("{{\"status\":\"rolled_back\",\"state\":\"{state}\"}}"))
        }
        Err(error) => error_response(&error),
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// The complete Workload OTA router, relative to the caller's namespace
/// (`.nest("/workload/ota", routes(...))`). Authorization and the activation
/// authority are injected; the service owns the (single) storage.
pub fn routes<A, Au, P>(service: &'static WorkloadOtaService<A>, auth: Au, control: P) -> HttpRouter<impl PathRouter>
where
    A: FlashAccess + 'static,
    Au: Authorize + Clone + 'static,
    P: ControlPort<A> + Clone + 'static,
{
    let status_auth = auth.clone();
    let prepare_auth = auth.clone();
    let activate_auth = auth.clone();
    let confirm_auth = auth.clone();
    let rollback_auth = auth.clone();
    let status_control = control.clone();
    let activate_control = control.clone();
    let confirm_control = control.clone();
    HttpRouter::new()
        .route("/status", get(move |Bearer(token): Bearer| {
            let auth = status_auth.clone();
            let control = status_control.clone();
            async move {
                if !auth.authorize(token.as_deref().unwrap_or("")).await {
                    return unauthorized();
                }
                json_ok(status_json(service.status().await, control.runtime_status().await))
            }
        }))
        .route("/prepare", post(move |Bearer(token): Bearer, body: String| {
            let auth = prepare_auth.clone();
            async move {
                if !auth.authorize(token.as_deref().unwrap_or("")).await {
                    return unauthorized();
                }
                prepare_response(service, &body).await
            }
        }))
        .route("/write", put_service(WorkloadWrite { service, auth }))
        .route("/activate", post(move |Bearer(token): Bearer, body: String| {
            let auth = activate_auth.clone();
            let control = activate_control.clone();
            async move {
                if !auth.authorize(token.as_deref().unwrap_or("")).await {
                    return unauthorized();
                }
                activate_response(service, &control, &body).await
            }
        }))
        .route("/confirm", post(move |Bearer(token): Bearer| {
            let auth = confirm_auth.clone();
            let control = confirm_control.clone();
            async move {
                if !auth.authorize(token.as_deref().unwrap_or("")).await {
                    return unauthorized();
                }
                confirm_response(service, &control).await
            }
        }))
        .route("/rollback", post(move |Bearer(token): Bearer| {
            let auth = rollback_auth.clone();
            let control = control.clone();
            async move {
                if !auth.authorize(token.as_deref().unwrap_or("")).await {
                    return unauthorized();
                }
                rollback_response(service, &control).await
            }
        }))
}

/// Streaming `PUT /write`. Like the Agent's: one chunk per request, `Content-Range`
/// resume/resync, the body never buffered whole.
pub struct WorkloadWrite<A: 'static, Au> {
    pub service: &'static WorkloadOtaService<A>,
    pub auth: Au,
}

impl<A: FlashAccess, Au> ChunkSink for WorkloadWrite<A, Au> {
    type Error = ();

    async fn write_chunk(&self, chunk: &[u8]) -> Result<(), Self::Error> {
        self.service.chunk(chunk).await.then_some(()).ok_or(())
    }
}

impl<A: FlashAccess, Au: Authorize> RequestHandlerService for WorkloadWrite<A, Au> {
    async fn call_request_handler_service<R: Read, W: ResponseWriter<Error = R::Error>>(
        &self,
        _state: &(),
        _path_parameters: (),
        mut request: Request<'_, R>,
        response_writer: W,
    ) -> Result<ResponseSent, W::Error> {
        let headers = request.parts.headers();
        let token = bearer_token(headers.get("authorization").and_then(|v| v.as_str().ok()));
        if !self.auth.authorize(token).await {
            return unauthorized().write_to(request.body_connection.finalize().await?, response_writer).await;
        }

        let digest_header = headers.get("x-embewi-digest").and_then(|v| v.as_str().ok()).unwrap_or("");
        let content_range = headers.get("content-range").and_then(|v| v.as_str().ok());
        let content_length = request.body_connection.content_length();

        // Refuse before touching the session: an invalid PUT must not disturb one in progress.
        let Some(digest) = parse_digest(digest_header) else {
            return bad_request("bad_digest")
                .write_to(request.body_connection.finalize().await?, response_writer)
                .await;
        };
        let (has_range, start, end, total) = match content_range {
            None => match u64::try_from(content_length).ok().filter(|len| *len > 0) {
                Some(len) => (false, 0u64, len - 1, len),
                None => {
                    return bad_request("empty_body")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
            },
            Some(value) => match parse_content_range(value) {
                Some(range) if range.len() == Some(content_length as u64) => (true, range.start, range.end, range.total),
                Some(_) => {
                    return bad_request("content_length_mismatch")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
                None => {
                    return bad_request("bad_content_range")
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
            },
        };

        match resume_plan(has_range, start, self.service.in_progress(), self.service.received()) {
            ResumePlan::Begin => {
                if let Err(error) = self.service.begin(&digest, total).await {
                    return error_response(&error)
                        .write_to(request.body_connection.finalize().await?, response_writer)
                        .await;
                }
            }
            ResumePlan::Resync => {
                let written = self.service.written();
                return json_error(
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    &format!("{{\"error\":\"range_mismatch\",\"written\":{written}}}"),
                )
                .write_to(request.body_connection.finalize().await?, response_writer)
                .await;
            }
            ResumePlan::Continue => {
                if !self.service.params_match(&digest, total) {
                    return error_response(&ServiceError::SessionMismatch)
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
                return error_response(&ServiceError::Storage)
                    .write_to(request.body_connection.finalize().await?, response_writer)
                    .await;
            }
        }

        if !is_complete(has_range, end, total) {
            let written = self.service.written();
            return json_ok(format!("{{\"status\":\"partial\",\"written\":{written}}}"))
                .write_to(request.body_connection.finalize().await?, response_writer)
                .await;
        }

        let response = match self.service.finish().await {
            Ok(staged) => {
                let digest = format_digest(&iobewi_ota::Digest(staged.digest));
                log::info!("workload ota: staged digest={digest} size={}", staged.written);
                json_ok(format!("{{\"status\":\"staged\",\"written\":{},\"digest\":\"{digest}\"}}", staged.written))
            }
            Err(error) => {
                if matches!(error, ServiceError::DigestMismatch(_)) {
                    log::warn!("workload ota: digest mismatch, never staged");
                }
                error_response(&error)
            }
        };
        response.write_to(request.body_connection.finalize().await?, response_writer).await
    }
}
