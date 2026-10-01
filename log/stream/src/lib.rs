#![no_std]

//! Best-effort WebSocket streaming of the lines captured by `iobewi-log`,
//! over a platform supplied secure transport. Failed connections discard
//! stale logs; logging never waits on network I/O.

extern crate alloc;

use alloc::{format, string::String};
use core::fmt::{Debug, Display};
use embassy_time::{with_timeout, Duration, Instant, Timer};
use embedded_io_async::{ErrorType, Read, Write};
use iobewi_log::{discard, pop_line, LogMetadata, RING_CAPACITY};
use iobewi_net_tls_core::SecureClientTransport;
use log::{info, warn};
use serde::Serialize;

const DRAIN_PERIOD: Duration = Duration::from_millis(200);
const FRAME_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const BASE_BACKOFF: Duration = Duration::from_secs(5);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const STABLE_THRESHOLD: Duration = Duration::from_secs(30);

/// Where and how to stream logs to the Core. The service never knows the
/// application's persistence schema or endpoint version.
#[allow(async_fn_in_trait)]
pub trait StreamConfig {
    async fn ctrl_url(&self) -> String;
    async fn token(&self) -> String;
    fn path(&self) -> String;
}

/// Randomness needed by the WebSocket protocol layer: the handshake nonce
/// and RFC 6455 client-frame masking. Not a property of the secure
/// transport itself (a plain HTTPS client, e.g. the heartbeat, never needs
/// this) -- its own narrow capability, supplied to [`run`] separately from
/// the transport.
#[allow(async_fn_in_trait)]
pub trait Entropy {
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

fn websocket_authority(host: &str, port: u16) -> String {
    if port == 443 { String::from(host) } else { format!("{host}:{port}") }
}

fn next_backoff(current: Duration) -> Duration {
    Duration::from_secs((current.as_secs() * 2).min(MAX_BACKOFF.as_secs()))
}

fn jittered(base: Duration, random: u32) -> Duration {
    let base_ms = base.as_millis() as i64;
    let percent = (random % 61) as i64 - 30;
    Duration::from_millis((base_ms + base_ms * percent / 100).max(1000) as u64)
}

async fn connect_and_upgrade<'a, T: SecureClientTransport, E: Entropy>(
    transport: &'a T,
    entropy: &E,
    rx: &'a mut [u8],
    tx: &'a mut [u8],
    host: &'a str,
    port: u16,
    path: &str,
    token: &str,
) -> Result<T::Connection<'a>, String> {
    let mut session = transport
        .connect(host, port, rx, tx)
        .await
        .map_err(|e| format!("connect failed: {e}"))?;
    let mut nonce = [0u8; iobewi_http::websocket::NONCE_LENGTH];
    entropy.random_bytes(&mut nonce);
    let authority = websocket_authority(host, port);
    iobewi_http::websocket::upgrade(&mut session, &authority, path, token, &nonce).await?;
    info!("logs: connected to {host}:{port}");
    Ok(session)
}

async fn pump_session<S: ErrorType + Read + Write, C: StreamConfig + LogMetadata, E: Entropy>(
    session: &mut S,
    config: &C,
    entropy: &E,
    token_snapshot: &str,
) -> String
where S::Error: Display + Debug {
    loop {
        if config.token().await != token_snapshot {
            return String::from("bearer token changed, reconnecting");
        }
        let mut first = [0u8; 1];
        match with_timeout(DRAIN_PERIOD, session.read(&mut first)).await {
            Err(_) => {} // Idle; no WebSocket frame byte was consumed.
            Ok(Ok(0)) => return String::from("server closed the connection"),
            Ok(Err(error)) => return format!("frame read failed: {error}"),
            Ok(Ok(_)) => match with_timeout(FRAME_TIMEOUT,
                iobewi_http::websocket::process_frame_after_first(&mut *session, first[0], entropy.random_u32())
            ).await {
                Ok(Ok(true)) => {},
                Ok(Ok(false)) => return String::from("server closed the connection"),
                Ok(Err(error)) => return error,
                Err(_) => return String::from("incomplete WebSocket frame timed out"),
            },
        }
        for _ in 0..RING_CAPACITY {
            let Some(line) = pop_line() else { break; };
            let node_id = config.node_id().await;
            let frame = LogFrame {
                ts: config.timestamp(), node: &node_id,
                workload: config.workload(), level: "raw", msg: &line,
            };
            let Ok(json) = serde_json::to_vec(&frame) else { continue; };
            if let Err(error) = iobewi_http::websocket::send_text(&mut *session, &json, entropy.random_u32()).await {
                return error;
            }
        }
    }
}

/// Reconnect with capped jittered backoff. Fresh logs are sent only while a
/// connection is active; the ring is cleared on every failed session.
pub async fn run<C: StreamConfig + LogMetadata, T: SecureClientTransport, E: Entropy>(config: &C, transport: &T, entropy: &E) -> ! {
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
        let path = config.path();
        let connected = with_timeout(HANDSHAKE_TIMEOUT,
            connect_and_upgrade(transport, entropy, &mut rx, &mut tx, host, port, &path, &token)
        ).await.unwrap_or_else(|_| Err(String::from("WebSocket handshake timed out")));
        let stable = match connected {
            Ok(mut session) => {
                let connected_at = Instant::now();
                let error = pump_session(&mut session, config, entropy, &token).await;
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
        Timer::after(jittered(backoff, entropy.random_u32())).await;
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
        assert_eq!(websocket_authority("core.example", 8443), "core.example:8443");
        assert_eq!(websocket_authority("core.example", 443), "core.example");
        assert_eq!(next_backoff(Duration::from_secs(40)), Duration::from_secs(60));
        assert_eq!(jittered(Duration::from_secs(5), 0), Duration::from_millis(3500));
    }
}
