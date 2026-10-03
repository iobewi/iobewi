//! End-to-end route tests: a real picoserve router over an in-memory connection,
//! raw HTTP bytes in, raw HTTP bytes out, on a fake NOR flash.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;
use core::future::Future;
use core::task::{Context, Poll, Waker};

use embedded_io_async::{ErrorType, Read, Write};
use iobewi_http_server::io_socket::IoSocket;
use iobewi_http_server::HttpRouter;
use iobewi_net_io::Close;
use iobewi_update_model::RuntimeApi;
use iobewi_workload_ota::flash::WorkloadFlash;
use iobewi_workload_ota::layout::{Region, Unsupported, assemble};
use iobewi_workload_ota::service::{Availability, WorkloadOtaService};
use iobewi_workload_ota::supervisor::{Health, WorkloadSupervisor};
use iobewi_workload_ota::testing::{ERASE, FakeAccess, FakeRuntime};
use sha2::{Digest as _, Sha256};

use crate::{Authorize, ControlPort, NoSupervisor, routes};

const TOKEN: &str = "good-token";
const FLASH: u32 = 0x40_0000;
const API10: RuntimeApi = RuntimeApi::new(1, 0);

// ---------- in-memory connection ----------

#[derive(Debug, Clone, Copy)]
struct Reset;
impl core::fmt::Display for Reset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "reset")
    }
}
impl core::error::Error for Reset {}
impl embedded_io_async::Error for Reset {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        embedded_io_async::ErrorKind::ConnectionReset
    }
}

#[derive(Default)]
struct State {
    input: VecDeque<u8>,
    output: Vec<u8>,
}

#[derive(Clone, Default)]
struct Conn(Rc<RefCell<State>>);

impl ErrorType for Conn {
    type Error = Reset;
}
impl Read for Conn {
    async fn read(&mut self, out: &mut [u8]) -> Result<usize, Reset> {
        let mut st = self.0.borrow_mut();
        let n = out.len().min(st.input.len());
        for slot in out.iter_mut().take(n) {
            *slot = st.input.pop_front().unwrap();
        }
        Ok(n)
    }
}
impl Write for Conn {
    async fn write(&mut self, data: &[u8]) -> Result<usize, Reset> {
        self.0.borrow_mut().output.extend_from_slice(data);
        Ok(data.len())
    }
    async fn flush(&mut self) -> Result<(), Reset> {
        Ok(())
    }
}
impl Close for Conn {
    async fn close(&mut self) -> Result<(), Reset> {
        Ok(())
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

// ---------- fixtures ----------

#[derive(Clone)]
struct Auth;
impl Authorize for Auth {
    async fn authorize(&self, token: &str) -> bool {
        token == TOKEN
    }
}

type Real = &'static WorkloadSupervisor<FakeAccess, FakeRuntime>;

/// A real Supervisor (over the fake runtime) as the control authority.
fn supervisor(svc: &'static WorkloadOtaService<FakeAccess>) -> Real {
    Box::leak(Box::new(WorkloadSupervisor::new(svc, FakeRuntime::new())))
}

fn layout() -> iobewi_workload_ota::layout::WorkloadLayout {
    assemble(
        Some(Region::new(0x320000, 0x2000)),
        Some(Region::new(0x322000, 0x6F000)),
        Some(Region::new(0x391000, 0x6F000)),
        ERASE,
    )
    .unwrap()
}

fn service(provided: RuntimeApi) -> &'static WorkloadOtaService<FakeAccess> {
    Box::leak(Box::new(WorkloadOtaService::new(
        Availability::Supported(WorkloadFlash::new(layout(), FakeAccess::new(FLASH))),
        provided,
    )))
}

fn unsupported_service() -> &'static WorkloadOtaService<FakeAccess> {
    Box::leak(Box::new(WorkloadOtaService::new(Availability::Unsupported(Unsupported::MissingMeta), API10)))
}

fn snapshot(svc: &WorkloadOtaService<FakeAccess>) -> Vec<u8> {
    svc.storage().unwrap().access().0.borrow().data.clone()
}

// ---------- raw HTTP ----------

struct Resp {
    status: u16,
    body: String,
}

impl Resp {
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("body is not JSON ({e}): {}", self.body))
    }
}

fn request(method: &str, path: &str, token: Option<&str>, extra: &[(&str, String)], body: &[u8]) -> Vec<u8> {
    let mut out = std::format!("{method} /workload/ota{path} HTTP/1.1\r\nHost: dev\r\n");
    if let Some(t) = token {
        out += &std::format!("Authorization: Bearer {t}\r\n");
    }
    for (k, v) in extra {
        out += &std::format!("{k}: {v}\r\n");
    }
    if !body.is_empty() || method != "GET" {
        out += &std::format!("Content-Length: {}\r\n", body.len());
    }
    out += "\r\n";
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

/// Several requests on ONE connection (keep-alive), responses parsed in order.
fn exchange<A: ControlPort<FakeAccess> + Clone + 'static>(
    svc: &'static WorkloadOtaService<FakeAccess>,
    port: A,
    requests: Vec<Vec<u8>>,
) -> Vec<Resp> {
    let router = HttpRouter::new().nest("/workload/ota", routes(svc, Auth, port));
    let config = iobewi_http_server::server_config();
    let conn = Conn::default();
    let state = conn.0.clone();
    for r in &requests {
        state.borrow_mut().input.extend(r.iter().copied());
    }
    let mut buffer = [0u8; iobewi_http_server::HTTP_BUFFER_LEN];
    block_on(iobewi_http_server::serve_one(&router, &config, &mut buffer, IoSocket::new(conn)));
    let out = state.borrow().output.clone();
    parse_responses(&out)
}

fn parse_responses(mut bytes: &[u8]) -> Vec<Resp> {
    let mut all = Vec::new();
    while !bytes.is_empty() {
        let head_end = bytes.windows(4).position(|w| w == b"\r\n\r\n").expect("response header end") + 4;
        let head = core::str::from_utf8(&bytes[..head_end]).unwrap();
        let status: u16 = head.split(' ').nth(1).unwrap().parse().unwrap();
        let len = head
            .lines()
            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
            .unwrap_or(0);
        let body = String::from_utf8_lossy(&bytes[head_end..head_end + len]).into_owned();
        all.push(Resp { status, body });
        bytes = &bytes[head_end + len..];
    }
    all
}

fn digest_of(bytes: &[u8]) -> String {
    let d: [u8; 32] = Sha256::digest(bytes).into();
    let mut s = String::from("sha256:");
    for b in d {
        s += &std::format!("{b:02x}");
    }
    s
}

fn prepare_body(version: &str, bytes: &[u8], api: (u16, u16)) -> Vec<u8> {
    std::format!(
        "{{\"artifact_id\":\"pod\",\"version\":\"{version}\",\"size\":{},\"digest\":\"{}\",\"required_runtime_api\":{{\"major\":{},\"minor\":{}}}}}",
        bytes.len(),
        digest_of(bytes),
        api.0,
        api.1
    )
    .into_bytes()
}

fn data(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| (i as u8).wrapping_mul(29).wrapping_add(seed)).collect()
}

fn put_chunk(bytes: &[u8], start: usize, end: usize, token: Option<&str>) -> Vec<u8> {
    let total = bytes.len();
    request(
        "PUT",
        "/write",
        token,
        &[
            ("X-Embewi-Digest", digest_of(bytes)),
            ("Content-Range", std::format!("bytes {}-{}/{total}", start, end - 1)),
        ],
        &bytes[start..end],
    )
}

fn upload_requests(bytes: &[u8], version: &str, api: (u16, u16), chunk: usize) -> Vec<Vec<u8>> {
    let mut reqs = alloc::vec![request("POST", "/prepare", Some(TOKEN), &[], &prepare_body(version, bytes, api))];
    let mut at = 0;
    while at < bytes.len() {
        let end = (at + chunk).min(bytes.len());
        reqs.push(put_chunk(bytes, at, end, Some(TOKEN)));
        at = end;
    }
    reqs
}

// ---------- tests ----------

#[test]
fn every_route_requires_the_bearer_token_and_https_only_is_the_servers_job() {
    let svc = service(API10);
    let before = snapshot(svc);
    let cases: [(&str, &str, Vec<u8>); 6] = [
        ("GET", "/status", Vec::new()),
        ("POST", "/prepare", b"{}".to_vec()),
        ("POST", "/activate", b"{}".to_vec()),
        ("POST", "/confirm", Vec::new()),
        ("POST", "/rollback", Vec::new()),
        ("PUT", "/write", b"x".to_vec()),
    ];
    for token in [None, Some("wrong-token"), Some("")] {
        for (method, path, body) in &cases {
            let extra: Vec<(&str, String)> = if *path == "/write" {
                alloc::vec![("X-Embewi-Digest", digest_of(b"x"))]
            } else {
                Vec::new()
            };
            let r = &exchange(svc, NoSupervisor, alloc::vec![request(method, path, token, &extra, body)])[0];
            assert_eq!(r.status, 401, "{method} {path} token {token:?}");
            assert_eq!(r.json()["error"], "unauthorized");
        }
    }
    assert_eq!(snapshot(svc), before, "unauthenticated calls must not touch the flash");
}

#[test]
fn status_is_descriptive_and_never_requires_a_slot() {
    let svc = service(API10);
    let r = &exchange(svc, NoSupervisor, alloc::vec![request("GET", "/status", Some(TOKEN), &[], b"")])[0];
    assert_eq!(r.status, 200);
    let j = r.json();
    assert_eq!(j["supported"], true);
    assert_eq!(j["state"], "none");
    assert_eq!(j["max_artifact_size"], 0x6F000);
    assert_eq!(j["runtime_api_provided"]["major"], 1);
    assert!(j["active"].is_null() && j["candidate"].is_null());
}

#[test]
fn an_unsupported_device_answers_status_200_and_everything_else_409() {
    let svc = unsupported_service();
    let before = alloc::vec![0u8];
    let _ = before;
    let bytes = data(1, 3000);
    let rs = exchange(
        svc,
        NoSupervisor,
        alloc::vec![
            request("GET", "/status", Some(TOKEN), &[], b""),
            request("POST", "/prepare", Some(TOKEN), &[], &prepare_body("1", &bytes, (1, 0))),
            put_chunk(&bytes, 0, 3000, Some(TOKEN)),
            request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(&bytes)).as_bytes()),
        ],
    );
    assert_eq!(rs[0].status, 200);
    assert_eq!(rs[0].json()["supported"], false);
    assert_eq!(rs[0].json()["reason"], "MissingMeta");
    for r in &rs[1..] {
        assert_eq!(r.status, 409, "{}", r.body);
        assert_eq!(r.json()["error"], "workload_storage_unsupported");
        assert_eq!(r.json()["reason"], "MissingMeta");
    }
}

#[test]
fn prepare_write_and_finish_stage_a_verified_workload() {
    let svc = service(API10);
    let bytes = data(1, 50_000);
    let rs = exchange(svc, NoSupervisor, upload_requests(&bytes, "1.2.0", (1, 0), 16 * 1024));
    assert_eq!(rs[0].status, 200);
    assert_eq!(rs[0].json()["accepted"], true);
    // 50_000 B = 16 KiB x 3 + remainder: 4 chunks; first three partial, last staged.
    for r in &rs[1..rs.len() - 1] {
        assert_eq!(r.status, 200);
        assert_eq!(r.json()["status"], "partial");
    }
    let last = rs.last().unwrap();
    assert_eq!(last.status, 200, "{}", last.body);
    assert_eq!(last.json()["status"], "staged");
    assert_eq!(last.json()["written"], 50_000);
    assert_eq!(last.json()["digest"], digest_of(&bytes));

    let s = &exchange(svc, NoSupervisor, alloc::vec![request("GET", "/status", Some(TOKEN), &[], b"")])[0];
    let j = s.json();
    assert_eq!(j["state"], "staged");
    assert_eq!(j["candidate"]["id"], "pod");
    assert_eq!(j["candidate"]["version"], "1.2.0");
    assert_eq!(j["candidate"]["size"], 50_000);
    assert_eq!(j["candidate"]["digest"], digest_of(&bytes));
    assert_eq!(j["candidate"]["required_runtime_api"]["minor"], 0);
}

#[test]
fn a_wrong_digest_gets_422_and_is_never_staged() {
    let svc = service(API10);
    let good = data(1, 20_000);
    let evil = data(2, 20_000);
    // Prepared for `good`, but the digest header claims `good` while the bytes are `evil`.
    let mut reqs = alloc::vec![request("POST", "/prepare", Some(TOKEN), &[], &prepare_body("1", &good, (1, 0)))];
    let total = evil.len();
    reqs.push(request(
        "PUT",
        "/write",
        Some(TOKEN),
        &[("X-Embewi-Digest", digest_of(&good)), ("Content-Range", std::format!("bytes 0-{}/{total}", total - 1))],
        &evil,
    ));
    reqs.push(request("GET", "/status", Some(TOKEN), &[], b""));
    let rs = exchange(svc, NoSupervisor, reqs);
    assert_eq!(rs[1].status, 422, "{}", rs[1].body);
    assert_eq!(rs[1].json()["error"], "digest_mismatch");
    assert_eq!(rs[1].json()["computed"], digest_of(&evil));
    assert_eq!(rs[2].json()["state"], "none");
    assert!(rs[2].json()["candidate"].is_null());
}

#[test]
fn oversize_is_413_at_prepare_with_nothing_erased() {
    let svc = service(API10);
    let before = snapshot(svc);
    let body = std::format!(
        "{{\"artifact_id\":\"pod\",\"version\":\"1\",\"size\":{},\"digest\":\"{}\",\"required_runtime_api\":{{\"major\":1,\"minor\":0}}}}",
        0x6F000 + 1,
        digest_of(b"x")
    );
    let r = &exchange(svc, NoSupervisor, alloc::vec![request("POST", "/prepare", Some(TOKEN), &[], body.as_bytes())])[0];
    assert_eq!(r.status, 413);
    assert_eq!(r.json()["error"], "size_too_large");
    assert_eq!(r.json()["max"], 0x6F000);
    assert_eq!(snapshot(svc), before);
}

#[test]
fn malformed_requests_are_400() {
    let svc = service(API10);
    let bytes = data(1, 2_000);
    let cases: [(&str, Vec<u8>, &str); 5] = [
        ("/prepare", b"not json".to_vec(), "bad_request"),
        ("/prepare", b"{\"artifact_id\":\"p\"}".to_vec(), "bad_request"),
        (
            "/prepare",
            b"{\"artifact_id\":\"p\",\"version\":\"1\",\"size\":10,\"digest\":\"md5:00\",\"required_runtime_api\":{\"major\":1,\"minor\":0}}".to_vec(),
            "bad_digest",
        ),
        ("/activate", b"{}".to_vec(), "missing_digest"),
        ("/activate", b"{\"digest\":\"zz\"}".to_vec(), "bad_digest"),
    ];
    for (path, body, error) in cases {
        let r = &exchange(svc, NoSupervisor, alloc::vec![request("POST", path, Some(TOKEN), &[], &body)])[0];
        assert_eq!(r.status, 400, "{path} {}", r.body);
        assert_eq!(r.json()["error"], error);
    }
    // Write: bad digest header, bad Content-Range, length mismatch, empty body.
    let bad_digest = request("PUT", "/write", Some(TOKEN), &[("X-Embewi-Digest", "sha256:12".into())], b"x");
    let bad_range = request("PUT", "/write", Some(TOKEN), &[("X-Embewi-Digest", digest_of(&bytes)), ("Content-Range", "bytes 5-1/10".into())], b"x");
    let mismatch = request("PUT", "/write", Some(TOKEN), &[("X-Embewi-Digest", digest_of(&bytes)), ("Content-Range", "bytes 0-9/100".into())], b"xx");
    let empty = request("PUT", "/write", Some(TOKEN), &[("X-Embewi-Digest", digest_of(&bytes))], b"");
    let rs = exchange(svc, NoSupervisor, alloc::vec![bad_digest, bad_range, mismatch, empty]);
    let errors: Vec<(u16, String)> = rs.iter().map(|r| (r.status, r.json()["error"].as_str().unwrap().to_string())).collect();
    assert_eq!(
        errors,
        [
            (400, "bad_digest".into()),
            (400, "bad_content_range".into()),
            (400, "content_length_mismatch".into()),
            (400, "empty_body".into())
        ]
    );
}

#[test]
fn write_without_prepare_or_with_the_wrong_artifact_is_409() {
    let svc = service(API10);
    let bytes = data(1, 8_192);
    let r = &exchange(svc, NoSupervisor, alloc::vec![put_chunk(&bytes, 0, 4096, Some(TOKEN))])[0];
    assert_eq!(r.status, 409);
    assert_eq!(r.json()["error"], "not_prepared");
    // Prepared for one artifact, upload of another (digest header differs).
    let other = data(9, 8_192);
    let rs = exchange(
        svc,
        NoSupervisor,
        alloc::vec![
            request("POST", "/prepare", Some(TOKEN), &[], &prepare_body("1", &bytes, (1, 0))),
            put_chunk(&other, 0, 4096, Some(TOKEN)),
        ],
    );
    assert_eq!(rs[1].status, 409);
    assert_eq!(rs[1].json()["error"], "session_mismatch");
}

#[test]
fn resume_resync_and_range_errors() {
    let svc = service(API10);
    let bytes = data(3, 64 * 1024);
    // Connection 1: prepare + first two 16 KiB chunks, then it "drops".
    let first = exchange(svc, NoSupervisor, {
        let mut r = alloc::vec![request("POST", "/prepare", Some(TOKEN), &[], &prepare_body("1", &bytes, (1, 0)))];
        r.push(put_chunk(&bytes, 0, 16 * 1024, Some(TOKEN)));
        r.push(put_chunk(&bytes, 16 * 1024, 32 * 1024, Some(TOKEN)));
        r
    });
    assert_eq!(first[2].json()["status"], "partial");
    assert_eq!(first[2].json()["written"], 32 * 1024);
    // Connection 2: a gap (skips ahead) -> 416 with the resume point; an overlap too.
    let gap = exchange(svc, NoSupervisor, alloc::vec![put_chunk(&bytes, 48 * 1024, 64 * 1024, Some(TOKEN))]);
    assert_eq!(gap[0].status, 416);
    assert_eq!(gap[0].json()["error"], "range_mismatch");
    assert_eq!(gap[0].json()["written"], 32 * 1024);
    let overlap = exchange(svc, NoSupervisor, alloc::vec![put_chunk(&bytes, 8 * 1024, 24 * 1024, Some(TOKEN))]);
    assert_eq!(overlap[0].status, 416);
    // Out-of-bounds range is rejected as malformed (end >= total).
    let oob = request(
        "PUT",
        "/write",
        Some(TOKEN),
        &[("X-Embewi-Digest", digest_of(&bytes)), ("Content-Range", std::format!("bytes 32768-70000/{}", bytes.len()))],
        b"x",
    );
    assert_eq!(exchange(svc, NoSupervisor, alloc::vec![oob])[0].status, 400);
    // Resume from the reported point and finish: the final digest is correct.
    let resumed = exchange(
        svc,
        NoSupervisor,
        alloc::vec![
            put_chunk(&bytes, 32 * 1024, 48 * 1024, Some(TOKEN)),
            put_chunk(&bytes, 48 * 1024, 64 * 1024, Some(TOKEN)),
        ],
    );
    assert_eq!(resumed[1].status, 200);
    assert_eq!(resumed[1].json()["status"], "staged");
    assert_eq!(resumed[1].json()["digest"], digest_of(&bytes));
}

#[test]
fn a_session_with_other_parameters_is_a_conflict() {
    let svc = service(API10);
    let bytes = data(3, 32 * 1024);
    let other_total = {
        // Same digest header but a different total than prepared.
        request(
            "PUT",
            "/write",
            Some(TOKEN),
            &[("X-Embewi-Digest", digest_of(&bytes)), ("Content-Range", "bytes 16384-32767/40000".into())],
            &bytes[16384..],
        )
    };
    let rs = exchange(
        svc,
        NoSupervisor,
        alloc::vec![
            request("POST", "/prepare", Some(TOKEN), &[], &prepare_body("1", &bytes, (1, 0))),
            put_chunk(&bytes, 0, 16 * 1024, Some(TOKEN)),
            other_total,
        ],
    );
    assert_eq!(rs[2].status, 409);
    assert_eq!(rs[2].json()["error"], "session_mismatch");
}

#[test]
fn a_new_prepare_supersedes_a_staged_candidate_but_not_a_running_one() {
    let svc = service(API10);
    let a = data(1, 6_000);
    let b = data(2, 7_000);
    let c = data(3, 5_000);
    let port = supervisor(svc);
    let rs = exchange(svc, port, {
        let mut r = upload_requests(&a, "1", (1, 0), 16 * 1024);
        r.extend(upload_requests(&b, "2", (1, 0), 16 * 1024));
        r.push(request("GET", "/status", Some(TOKEN), &[], b""));
        r
    });
    assert_eq!(rs.last().unwrap().json()["candidate"]["version"], "2");
    // Activate through the Supervisor: PendingConfirmation -> prepare refused.
    let rs = exchange(
        svc,
        port,
        alloc::vec![
            request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(&b)).as_bytes()),
            request("POST", "/prepare", Some(TOKEN), &[], &prepare_body("3", &c, (1, 0))),
        ],
    );
    assert_eq!(rs[0].status, 200, "{}", rs[0].body);
    assert_eq!(rs[1].status, 409);
    assert_eq!(rs[1].json()["error"], "workload_busy");
    assert_eq!(rs[1].json()["state"], "pending_confirmation");
}

#[test]
fn activation_without_a_supervisor_is_501_and_the_state_stays_staged() {
    let svc = service(API10);
    let bytes = data(1, 9_000);
    exchange(svc, NoSupervisor, upload_requests(&bytes, "1", (1, 0), 16 * 1024));
    let activate = request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(&bytes)).as_bytes());
    let before = snapshot(svc);
    let rs = exchange(
        svc,
        NoSupervisor,
        alloc::vec![activate.clone(), activate, request("GET", "/status", Some(TOKEN), &[], b"")],
    );
    for r in &rs[..2] {
        assert_eq!(r.status, 501);
        assert_eq!(r.json()["error"], "supervisor_unavailable");
    }
    assert_eq!(rs[2].json()["state"], "staged", "an unavailable supervisor must not move the state");
    assert_eq!(snapshot(svc), before, "no OTM2 write at all");
}

#[test]
fn activation_checks_come_before_the_supervisor_gate() {
    let svc = service(API10);
    let bytes = data(1, 9_000);
    let digest = digest_of(&bytes);
    // Nothing staged -> 409 not_staged (not 501).
    let none = &exchange(svc, NoSupervisor, alloc::vec![request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{digest}\"}}").as_bytes())])[0];
    assert_eq!(none.status, 409);
    assert_eq!(none.json()["error"], "not_staged");
    // Staged for runtime API 1.4 while the Agent provides 1.0: staging is allowed...
    let rs = exchange(svc, NoSupervisor, upload_requests(&bytes, "future", (1, 4), 16 * 1024));
    assert_eq!(rs.last().unwrap().json()["status"], "staged");
    // ...activation is refused (409), candidate kept.
    let refused = exchange(
        svc,
        NoSupervisor,
        alloc::vec![
            request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{digest}\"}}").as_bytes()),
            request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(b"other")).as_bytes()),
            request("GET", "/status", Some(TOKEN), &[], b""),
        ],
    );
    assert_eq!(refused[0].status, 409);
    assert_eq!(refused[0].json()["error"], "incompatible_runtime_api");
    assert_eq!(refused[0].json()["required"], "1.4");
    assert_eq!(refused[0].json()["provided"], "1.0");
    assert_eq!(refused[1].status, 409);
    assert_eq!(refused[2].json()["state"], "staged");
    assert_eq!(refused[2].json()["candidate"]["version"], "future");
}

#[test]
fn a_compatible_workload_activates_confirms_and_the_status_shows_it_running() {
    let svc = service(RuntimeApi::new(1, 4));
    let bytes = data(1, 9_000);
    let port = supervisor(svc);
    exchange(svc, port, upload_requests(&bytes, "1", (1, 3), 16 * 1024));
    let activate = request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(&bytes)).as_bytes());
    let rs = exchange(
        svc,
        port,
        alloc::vec![
            activate,
            request("GET", "/status", Some(TOKEN), &[], b""),
            request("POST", "/confirm", Some(TOKEN), &[], b""),
            request("GET", "/status", Some(TOKEN), &[], b""),
        ],
    );
    assert_eq!(rs[0].status, 200, "{}", rs[0].body);
    assert_eq!(rs[0].json()["status"], "pending_confirmation");
    assert_eq!(rs[1].json()["state"], "pending_confirmation");
    assert_eq!(rs[1].json()["runtime"]["supervised"], true);
    assert_eq!(rs[1].json()["runtime"]["running"], true);
    assert_eq!(rs[1].json()["runtime"]["health"], "healthy");
    assert_eq!(rs[1].json()["runtime"]["artifact"]["version"], "1");
    assert_eq!(rs[2].status, 200);
    assert_eq!(rs[2].json()["status"], "valid");
    assert_eq!(rs[3].json()["state"], "valid");
    assert_eq!(rs[3].json()["active"]["version"], "1");
}

#[test]
fn an_unhealthy_candidate_is_409_on_confirm_and_a_rollback_restores_the_previous() {
    let svc = service(API10);
    let a = data(1, 8_000);
    let b = data(2, 8_000);
    let port = supervisor(svc);
    let act = |bytes: &[u8]| request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(bytes)).as_bytes());
    exchange(svc, port, upload_requests(&a, "A", (1, 0), 16 * 1024));
    exchange(svc, port, alloc::vec![act(&a), request("POST", "/confirm", Some(TOKEN), &[], b"")]);
    exchange(svc, port, upload_requests(&b, "B", (1, 0), 16 * 1024));
    exchange(svc, port, alloc::vec![act(&b)]);
    port.runtime().set_health(Some(Health::Unhealthy));
    let rs = exchange(
        svc,
        port,
        alloc::vec![
            request("POST", "/confirm", Some(TOKEN), &[], b""),
            request("GET", "/status", Some(TOKEN), &[], b""),
        ],
    );
    assert_eq!(rs[0].status, 409);
    assert_eq!(rs[0].json()["error"], "workload_unhealthy");
    assert_eq!(rs[0].json()["health"], "unhealthy");
    assert_eq!(rs[1].json()["state"], "pending_confirmation");
    assert_eq!(rs[1].json()["runtime"]["health"], "unhealthy");
    port.runtime().set_health(None);
    let rs = exchange(
        svc,
        port,
        alloc::vec![
            request("POST", "/rollback", Some(TOKEN), &[], b""),
            request("GET", "/status", Some(TOKEN), &[], b""),
        ],
    );
    assert_eq!(rs[0].status, 200, "{}", rs[0].body);
    assert_eq!(rs[0].json()["status"], "rolled_back");
    assert_eq!(rs[0].json()["state"], "valid");
    assert_eq!(rs[1].json()["active"]["version"], "A");
    assert_eq!(rs[1].json()["runtime"]["artifact"]["version"], "A");
}

#[test]
fn a_start_failure_is_500_with_rolled_back_and_the_previous_workload_runs_again() {
    let svc = service(API10);
    let a = data(1, 8_000);
    let b = data(2, 8_000);
    let port = supervisor(svc);
    let act = |bytes: &[u8]| request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(bytes)).as_bytes());
    exchange(svc, port, upload_requests(&a, "A", (1, 0), 16 * 1024));
    exchange(svc, port, alloc::vec![act(&a), request("POST", "/confirm", Some(TOKEN), &[], b"")]);
    exchange(svc, port, upload_requests(&b, "B", (1, 0), 16 * 1024));
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&Sha256::digest(&b));
    port.runtime().fail_start_of(digest);
    let rs = exchange(svc, port, alloc::vec![act(&b), request("GET", "/status", Some(TOKEN), &[], b"")]);
    assert_eq!(rs[0].status, 500);
    assert_eq!(rs[0].json()["error"], "activation_failed");
    assert_eq!(rs[0].json()["rolled_back"], true);
    assert_eq!(rs[1].json()["state"], "valid");
    assert_eq!(rs[1].json()["active"]["version"], "A");
    assert_eq!(rs[1].json()["runtime"]["artifact"]["version"], "A");
}

#[test]
fn confirm_and_rollback_in_the_wrong_state_are_409_and_without_a_supervisor_501() {
    let svc = service(API10);
    let real = supervisor(svc);
    let rs = exchange(
        svc,
        real,
        alloc::vec![
            request("POST", "/confirm", Some(TOKEN), &[], b""),
            request("POST", "/rollback", Some(TOKEN), &[], b""),
        ],
    );
    assert_eq!((rs[0].status, rs[1].status), (409, 409));
    assert_eq!(rs[0].json()["error"], "not_staged");
    let rs = exchange(
        svc,
        NoSupervisor,
        alloc::vec![
            request("POST", "/confirm", Some(TOKEN), &[], b""),
            request("POST", "/rollback", Some(TOKEN), &[], b""),
            request("GET", "/status", Some(TOKEN), &[], b""),
        ],
    );
    assert_eq!((rs[0].status, rs[1].status), (501, 501));
    assert_eq!(rs[2].json()["runtime"]["supervised"], false);
}

#[test]
fn workload_routes_never_touch_the_agent_regions_or_otm1() {
    let svc = service(API10);
    let l = layout();
    let regions = l.regions();
    {
        let mut guard = svc.storage().unwrap().access().0.borrow_mut();
        for (i, b) in guard.data.iter_mut().enumerate() {
            if !regions.iter().any(|r| (i as u64) >= u64::from(r.offset) && (i as u64) < r.end()) {
                *b = (i % 251) as u8 | 1; // stands for ota_0, ota_1, otadata (OTM1), nvs, ...
            }
        }
    }
    let before = snapshot(svc);
    let bytes = data(5, 60_000);
    let a = data(6, 30_000);
    exchange(svc, NoSupervisor, upload_requests(&bytes, "1", (1, 0), 16 * 1024));
    exchange(svc, NoSupervisor, alloc::vec![request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(&bytes)).as_bytes())]);
    exchange(svc, NoSupervisor, upload_requests(&a, "2", (1, 0), 16 * 1024));
    let after = snapshot(svc);
    for (i, (x, y)) in before.iter().zip(after.iter()).enumerate() {
        if !regions.iter().any(|r| (i as u64) >= u64::from(r.offset) && (i as u64) < r.end()) {
            assert_eq!(x, y, "byte {i:#x} outside the Workload regions changed");
        }
    }
}

#[test]
fn a_failed_new_upload_leaves_no_staged_record_over_an_overwritten_slot() {
    let svc = service(API10);
    let old = data(1, 20_000);
    let evil = data(2, 20_000);
    let other = data(3, 20_000);
    // Stage `old`, then upload other bytes under another digest: 422, and `old` is gone too --
    // its slot was overwritten from the first byte, so it must not stay declared Staged.
    exchange(svc, NoSupervisor, upload_requests(&old, "old", (1, 0), 16 * 1024));
    let mut reqs = alloc::vec![request("POST", "/prepare", Some(TOKEN), &[], &prepare_body("new", &other, (1, 0)))];
    let total = evil.len();
    reqs.push(request(
        "PUT",
        "/write",
        Some(TOKEN),
        &[("X-Embewi-Digest", digest_of(&other)), ("Content-Range", std::format!("bytes 0-{}/{total}", total - 1))],
        &evil,
    ));
    reqs.push(request("GET", "/status", Some(TOKEN), &[], b""));
    let rs = exchange(svc, NoSupervisor, reqs);
    assert_eq!(rs[1].status, 422);
    assert_eq!(rs[2].json()["state"], "empty");
    assert!(rs[2].json()["candidate"].is_null(), "a Staged record over overwritten bytes is a lie");
}

#[cfg(feature = "test-fault-injection")]
#[test]
fn post_staging_corruption_is_refused_at_activation_and_the_previous_workload_keeps_running() {
    let svc = service(RuntimeApi::new(1, 4));
    let port = supervisor(svc);
    let a = data(1, 9_000);
    exchange(svc, port, upload_requests(&a, "A", (1, 3), 16 * 1024));
    let activate = |bytes: &[u8]| request("POST", "/activate", Some(TOKEN), &[], std::format!("{{\"digest\":\"{}\"}}", digest_of(bytes)).as_bytes());
    let rs = exchange(svc, port, alloc::vec![activate(&a), request("POST", "/confirm", Some(TOKEN), &[], b"")]);
    assert_eq!(rs[1].status, 200);
    let b = data(2, 12_000);
    exchange(svc, port, upload_requests(&b, "B", (1, 3), 16 * 1024));
    let rs = exchange(
        svc,
        port,
        alloc::vec![
            request("POST", "/test/corrupt-candidate", Some(TOKEN), &[], b"5000"),
            activate(&b),
            request("GET", "/status", Some(TOKEN), &[], b""),
        ],
    );
    assert_eq!(rs[0].status, 200, "{}", rs[0].body);
    assert_eq!(rs[1].status, 422, "{}", rs[1].body);
    assert_eq!(rs[1].json()["error"], "candidate_corrupted");
    assert_eq!(rs[2].json()["state"], "valid", "the corrupted candidate stopped existing");
    assert_eq!(rs[2].json()["runtime"]["artifact"]["version"], "A", "A was never stopped");
    // The route itself is authenticated.
    let rs = exchange(svc, port, alloc::vec![request("POST", "/test/corrupt-candidate", None, &[], b"1")]);
    assert_eq!(rs[0].status, 401);
}

#[cfg(not(feature = "test-fault-injection"))]
#[test]
fn the_fault_injection_route_does_not_exist_in_a_production_build() {
    let svc = service(RuntimeApi::new(1, 4));
    let rs = exchange(svc, supervisor(svc), alloc::vec![request("POST", "/test/corrupt-candidate", Some(TOKEN), &[], b"1")]);
    assert_eq!(rs[0].status, 404);
}
