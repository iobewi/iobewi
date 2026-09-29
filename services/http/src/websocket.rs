//! Outbound WebSocket upgrade and frame handling over a connected stream.
//! The platform provides the nonce and masking keys using its RNG.

use alloc::{format, string::String};
use core::fmt::Write as _;
use edge_http::ws::{is_upgrade_accepted, upgrade_request_headers, MAX_BASE64_KEY_LEN, MAX_BASE64_KEY_RESPONSE_LEN, NONCE_LEN};
use edge_ws::{FrameHeader, FrameType};
use embedded_io_async::{ErrorType, Read, Write};

pub const NONCE_LENGTH: usize = NONCE_LEN;

/// Complete an authenticated HTTP upgrade on an already connected stream.
pub async fn upgrade<S>(
    session: &mut S,
    host: &str,
    path: &str,
    bearer: &str,
    nonce: &[u8; NONCE_LEN],
) -> Result<(), String>
where
    S: Read + Write,
    S::Error: core::fmt::Display,
{
    if [host, path, bearer].iter().any(|v| v.contains('\r') || v.contains('\n')) || !path.starts_with('/') {
        return Err(String::from("invalid WebSocket request field"));
    }
    let mut key_buf = [0u8; MAX_BASE64_KEY_LEN];
    let origin = format!("https://{host}");
    let headers = upgrade_request_headers(Some(host), Some(&origin), None, nonce, &mut key_buf);
    let mut request = format!("GET {path} HTTP/1.1\r\n");
    for (name, value) in headers {
        if !name.is_empty() {
            let _ = write!(request, "{name}: {value}\r\n");
        }
    }
    let _ = write!(request, "Authorization: Bearer {bearer}\r\n\r\n");
    session.write_all(request.as_bytes()).await.map_err(|e| format!("upgrade request failed: {e}"))?;
    session.flush().await.map_err(|e| format!("upgrade flush failed: {e}"))?;

    let mut resp_buf = [0u8; 512];
    let mut filled = 0;
    let header_end = loop {
        if filled >= resp_buf.len() {
            return Err(String::from("upgrade response headers too large"));
        }
        let n = session.read(&mut resp_buf[filled..]).await.map_err(|e| format!("upgrade read failed: {e}"))?;
        if n == 0 { return Err(String::from("connection closed during upgrade")); }
        filled += n;
        if let Some(pos) = resp_buf[..filled].windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let mut httparse_headers = [httparse::EMPTY_HEADER; 24];
    let mut response = httparse::Response::new(&mut httparse_headers);
    match response.parse(&resp_buf[..header_end]) {
        Ok(httparse::Status::Complete(_)) => {},
        Ok(httparse::Status::Partial) => return Err(String::from("incomplete upgrade response")),
        Err(e) => return Err(format!("upgrade response parse failed: {e:?}")),
    }
    let code = response.code.ok_or_else(|| String::from("upgrade response has no status code"))?;
    let header_pairs: heapless::Vec<(&str, &str), 24> = response.headers.iter()
        .filter_map(|h| core::str::from_utf8(h.value).ok().map(|v| (h.name, v)))
        .collect();
    let mut accept_buf = [0u8; MAX_BASE64_KEY_RESPONSE_LEN];
    if !is_upgrade_accepted(code, header_pairs, nonce, &mut accept_buf) {
        return Err(String::from("server did not accept the WS upgrade"));
    }
    Ok(())
}

/// Process one inbound frame; `false` means the peer closed the WebSocket.
/// Callers may bound this operation with their scheduler's timeout.
pub async fn process_frame<S>(session: &mut S, mask_key: u32) -> Result<bool, String>
where
    S: Read + Write,
    S::Error: core::fmt::Debug,
{
    let mut header = FrameHeader::recv(&mut *session).await.map_err(|e| format!("recv failed: {e:?}"))?;
    let mut payload = [0u8; 125];
    let payload = header.recv_payload(&mut *session, &mut payload).await.map_err(|e| format!("recv_payload failed: {e:?}"))?;
    match header.frame_type {
        FrameType::Ping => {
            header.frame_type = FrameType::Pong;
            header.mask_key = Some(mask_key);
            header.send(&mut *session).await.map_err(|e| format!("pong send failed: {e:?}"))?;
            header.send_payload(&mut *session, payload).await.map_err(|e| format!("pong payload failed: {e:?}"))?;
        }
        FrameType::Close => return Ok(false),
        _ => {}
    }
    Ok(true)
}

/// Process a frame after the caller has read its first byte to distinguish
/// an idle connection from a partially received frame. Once any frame byte
/// arrives, callers must close the session if processing times out.
pub async fn process_frame_after_first<S>(session: &mut S, first: u8, mask_key: u32) -> Result<bool, String>
where
    S: Read + Write,
    S::Error: core::fmt::Debug,
{
    let mut prefixed = Prefixed { session, first: Some(first) };
    process_frame(&mut prefixed, mask_key).await
}

struct Prefixed<'a, S> {
    session: &'a mut S,
    first: Option<u8>,
}

impl<S: ErrorType> ErrorType for Prefixed<'_, S> {
    type Error = S::Error;
}

impl<S: Read> Read for Prefixed<'_, S> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if !buf.is_empty() {
            if let Some(first) = self.first.take() {
                buf[0] = first;
                return Ok(1);
            }
        }
        self.session.read(buf).await
    }
}

impl<S: Write> Write for Prefixed<'_, S> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.session.write(buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.session.flush().await
    }
}

/// Write one masked text frame. The platform must supply a fresh masking key.
pub async fn send_text<S>(session: &mut S, payload: &[u8], mask_key: u32) -> Result<(), String>
where
    S: Read + Write,
    S::Error: core::fmt::Debug,
{
    edge_ws::io::send(session, FrameType::Text(false), Some(mask_key), payload)
        .await.map_err(|e| format!("send failed: {e:?}"))
}
