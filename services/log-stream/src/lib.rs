#![no_std]

//! Bounded log capture and best-effort WebSocket streaming over a platform
//! supplied secure transport. Failed connections discard stale logs; logging
//! never waits on network I/O.

extern crate alloc;

use alloc::{ffi::CString, format, string::String};
use core::{cell::RefCell, ffi::CStr, fmt::{Debug, Display, Write as _}};
use critical_section::Mutex;
use embassy_time::{with_timeout, Duration, Instant, Timer};
use embedded_io_async::{Error, ErrorType, Read, Write};
use heapless::{Deque, String as FixedString};
use log::{Level, LevelFilter, Metadata, Record, info, warn};
use serde::Serialize;
use static_cell::StaticCell;

const LINE_MAX: usize = 160;
const RING_CAPACITY: usize = 24;
const DRAIN_PERIOD: Duration = Duration::from_millis(200);
const BASE_BACKOFF: Duration = Duration::from_secs(5);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const STABLE_THRESHOLD: Duration = Duration::from_secs(30);

static RING: Mutex<RefCell<Deque<FixedString<LINE_MAX>, RING_CAPACITY>>> =
    Mutex::new(RefCell::new(Deque::new()));
static LOGGER: StaticCell<Logger> = StaticCell::new();

struct Logger {
    print: fn(&Record<'_>),
    application_target: &'static str,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        if metadata.target().starts_with(self.application_target) {
            metadata.level() <= Level::Info
        } else {
            metadata.level() <= Level::Warn
        }
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) { return; }
        (self.print)(record);
        let mut line: FixedString<LINE_MAX> = FixedString::new();
        if write!(line, "{}", record.args()).is_err() { return; }
        critical_section::with(|cs| {
            let mut ring = RING.borrow(cs).borrow_mut();
            if !ring.is_full() { let _ = ring.push_back(line); }
        });
    }

    fn flush(&self) {}
}

/// Install once, during single-threaded startup, before other code logs.
/// The platform owns local console output and chooses the app log target.
pub fn install(print: fn(&Record<'_>), application_target: &'static str) {
    let logger = LOGGER.init(Logger { print, application_target });
    // SAFETY: the logger is process-lifetime storage and installation takes
    // place only once, before the executor and interrupt-driven loggers start.
    unsafe {
        let _ = log::set_logger_racy(logger);
        log::set_max_level_racy(LevelFilter::Info);
    }
}

fn pop_line() -> Option<FixedString<LINE_MAX>> {
    critical_section::with(|cs| RING.borrow(cs).borrow_mut().pop_front())
}

fn discard() {
    critical_section::with(|cs| RING.borrow(cs).borrow_mut().clear());
}

/// Application-supplied configuration and log metadata. The service never
/// knows the application's persistence schema or endpoint version.
#[allow(async_fn_in_trait)]
pub trait LogConfig {
    async fn ctrl_url(&self) -> String;
    async fn token(&self) -> String;
    async fn node_id(&self) -> String;
    fn timestamp(&self) -> u64;
    fn workload(&self) -> &'static str;
    fn path(&self) -> &'static str;
}

/// Platform-owned network, trusted TLS session and cryptographic randomness.
/// The platform must reject connections until the clock and CA are valid.
#[allow(async_fn_in_trait)]
pub trait Transport {
    type IoError: Error + Display + Debug;
    type Connection<'host, 'buffers>: ErrorType<Error = Self::IoError> + Read + Write;

    async fn connect<'host, 'buffers>(
        &self,
        host: &'host CStr,
        port: u16,
        rx: &'buffers mut [u8],
        tx: &'buffers mut [u8],
    ) -> Result<Self::Connection<'host, 'buffers>, String>;

    fn random_bytes(&self, output: &mut [u8]);
    fn random_u32(&self) -> u32;
}

#[derive(Serialize)]
struct LogFrame<'a> {
    ts: u64,
    node: &'a str,
    workload: &'static str,
    level: &'static str,
    msg: &'a str,
}

fn split_host_port(ctrl_url: &str) -> Option<(&str, u16)> {
    let rest = ctrl_url.split_once("://").map(|(_, value)| value).unwrap_or(ctrl_url);
    let host_port = rest.split('/').next().unwrap_or(rest);
    match host_port.split_once(':') {
        Some((host, port)) if !host.is_empty() => Some((host, port.parse().unwrap_or(443))),
        _ if !host_port.is_empty() => Some((host_port, 443)),
        _ => None,
    }
}

fn next_backoff(current: Duration) -> Duration {
    Duration::from_secs((current.as_secs() * 2).min(MAX_BACKOFF.as_secs()))
}

fn jittered(base: Duration, random: u32) -> Duration {
    let base_ms = base.as_millis() as i64;
    let percent = (random % 61) as i64 - 30;
    Duration::from_millis((base_ms + base_ms * percent / 100).max(1000) as u64)
}

async fn connect_and_upgrade<'host, 'buffers, T: Transport>(
    transport: &T,
    rx: &'buffers mut [u8],
    tx: &'buffers mut [u8],
    host: &'host CStr,
    port: u16,
    path: &str,
    token: &str,
) -> Result<T::Connection<'host, 'buffers>, String> {
    let mut session = transport.connect(host, port, rx, tx).await?;
    let host_str = host.to_str().unwrap_or("");
    let mut nonce = [0u8; iobewi_http::websocket::NONCE_LENGTH];
    transport.random_bytes(&mut nonce);
    iobewi_http::websocket::upgrade(&mut session, host_str, path, token, &nonce).await?;
    info!("logs: connected to {host_str}:{port}");
    Ok(session)
}

async fn pump_session<S: ErrorType + Read + Write, C: LogConfig, T: Transport>(
    session: &mut S,
    config: &C,
    transport: &T,
    token_snapshot: &str,
) -> String
where S::Error: Display + Debug {
    loop {
        if config.token().await != token_snapshot {
            return String::from("bearer token changed, reconnecting");
        }
        match with_timeout(DRAIN_PERIOD,
            iobewi_http::websocket::process_frame(&mut *session, transport.random_u32())
        ).await {
            Ok(Ok(true)) | Err(_) => {}
            Ok(Ok(false)) => return String::from("server closed the connection"),
            Ok(Err(error)) => return error,
        }
        while let Some(line) = pop_line() {
            let node_id = config.node_id().await;
            let frame = LogFrame {
                ts: config.timestamp(), node: &node_id,
                workload: config.workload(), level: "raw", msg: &line,
            };
            let Ok(json) = serde_json::to_vec(&frame) else { continue; };
            if let Err(error) = iobewi_http::websocket::send_text(&mut *session, &json, transport.random_u32()).await {
                return error;
            }
        }
    }
}

/// Reconnect with capped jittered backoff. Fresh logs are sent only while a
/// connection is active; the ring is cleared on every failed session.
pub async fn run<C: LogConfig, T: Transport>(config: &C, transport: &T) -> ! {
    let mut rx = [0u8; 1024];
    let mut tx = [0u8; 512];
    let mut backoff = BASE_BACKOFF;
    loop {
        let ctrl_url = config.ctrl_url().await;
        let Some((host, port)) = split_host_port(&ctrl_url) else {
            discard();
            Timer::after(BASE_BACKOFF).await;
            continue;
        };
        let token = config.token().await;
        if token.is_empty() {
            discard();
            Timer::after(BASE_BACKOFF).await;
            continue;
        }
        let Ok(host_c) = CString::new(host) else {
            discard();
            Timer::after(BASE_BACKOFF).await;
            continue;
        };
        let stable = match connect_and_upgrade(transport, &mut rx, &mut tx, &host_c, port, config.path(), &token).await {
            Ok(mut session) => {
                let connected_at = Instant::now();
                let error = pump_session(&mut session, config, transport, &token).await;
                warn!("logs: session ended: {error}");
                connected_at.elapsed() >= STABLE_THRESHOLD
            }
            Err(error) => {
                warn!("logs: session ended: {error}");
                false
            }
        };
        discard();
        if stable { backoff = BASE_BACKOFF; }
        Timer::after(jittered(backoff, transport.random_u32())).await;
        backoff = next_backoff(backoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_policy_and_url_parsing() {
        assert_eq!(split_host_port("https://core.example:8443/foo"), Some(("core.example", 8443)));
        assert_eq!(split_host_port("core.example"), Some(("core.example", 443)));
        assert_eq!(split_host_port(""), None);
        assert_eq!(next_backoff(Duration::from_secs(40)), Duration::from_secs(60));
        assert_eq!(jittered(Duration::from_secs(5), 0), Duration::from_millis(3500));
    }
}
