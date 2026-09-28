//! Outbound HTTP/1.1 primitives over any connected async transport.
//! TLS connection setup and certificate policy belong to the platform adapter.

use alloc::{format, string::String};
use embedded_io_async::{Read, Write};

/// Send a JSON POST while leaving the connection open for another request.
pub async fn post_json<S>(
    session: &mut S,
    host: &str,
    path: &str,
    bearer: &str,
    json: &str,
) -> Result<(), String>
where
    S: Read + Write,
    S::Error: core::fmt::Display,
{
    if [host, path, bearer].iter().any(|v| v.contains('\r') || v.contains('\n')) || !path.starts_with('/') {
        return Err(String::from("invalid HTTP request field"));
    }
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {bearer}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{json}",
        json.len()
    );
    session.write_all(request.as_bytes()).await.map_err(|e| format!("write failed: {e}"))?;
    session.flush().await.map_err(|e| format!("flush failed: {e}"))?;
    Ok(())
}

/// Consume exactly one response and report whether the stream can be reused.
/// The buffer must hold the complete response headers; bodies are discarded
/// incrementally, including Content-Length and chunked transfer encoding.
pub async fn drain_response<S>(
    session: &mut S,
    resp_buf: &mut [u8],
) -> Result<(u16, bool), String>
where
    S: Read + Write,
    S::Error: core::fmt::Display,
{
    let mut filled = 0;
    let header_end = loop {
        if filled >= resp_buf.len() {
            return Err(String::from("response headers too large for the scratch buffer"));
        }
        let n = session.read(&mut resp_buf[filled..]).await.map_err(|e| format!("read failed: {e}"))?;
        if n == 0 {
            return Err(String::from("connection closed while reading response headers"));
        }
        filled += n;
        if let Some(pos) = resp_buf[..filled].windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let mut httparse_headers = [httparse::EMPTY_HEADER; 16];
    let mut response = httparse::Response::new(&mut httparse_headers);
    match response.parse(&resp_buf[..header_end]) {
        Ok(httparse::Status::Complete(_)) => {}
        Ok(httparse::Status::Partial) => return Err(String::from("response headers unexpectedly incomplete")),
        Err(e) => return Err(format!("response parse failed: {e:?}")),
    }
    let status = response.code.unwrap_or(0);

    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    let mut keep_alive = true; // HTTP/1.1 default, absent a `Connection` header saying otherwise.
    for h in response.headers.iter() {
        let Ok(value) = core::str::from_utf8(h.value) else { continue };
        if h.name.eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().ok();
        } else if h.name.eq_ignore_ascii_case("transfer-encoding") {
            chunked = value.to_ascii_lowercase().contains("chunked");
        } else if h.name.eq_ignore_ascii_case("connection") && value.to_ascii_lowercase().contains("close") {
            keep_alive = false;
        }
    }

    // Small scratch space for discarding body bytes this loop has no use
    // for reading into `resp_buf` itself -- the point is just to advance
    // past them on the wire, not to keep them.
    let mut discard = [0u8; 128];

    if let Some(len) = content_length {
        let body_so_far = filled - header_end;
        let mut remaining = len.saturating_sub(body_so_far);
        while remaining > 0 {
            let to_read = remaining.min(discard.len());
            let n = session.read(&mut discard[..to_read]).await.map_err(|e| format!("body read failed: {e}"))?;
            if n == 0 {
                return Err(String::from("connection closed mid-body"));
            }
            remaining -= n;
        }
    } else if chunked {
        // Compacts whatever body bytes already arrived with the headers to
        // the front of `resp_buf`, then decodes in place: `carry_len` is
        // how many *unprocessed* bytes are sitting at `resp_buf[..carry_len]`
        // (chunk-size lines, chunk data, or trailers not yet consumed).
        // Capacity matches `resp_buf` exactly, so nothing already read off
        // the wire can be dropped the way a separately-sized buffer might.
        let mut carry_len = filled - header_end;
        resp_buf.copy_within(header_end..filled, 0);

        loop {
            let line_end = loop {
                if let Some(pos) = resp_buf[..carry_len].windows(2).position(|w| w == b"\r\n") {
                    break pos;
                }
                if carry_len >= resp_buf.len() {
                    return Err(String::from("chunk size line too long for the scratch buffer"));
                }
                let n = session.read(&mut resp_buf[carry_len..]).await.map_err(|e| format!("chunk read failed: {e}"))?;
                if n == 0 {
                    return Err(String::from("connection closed mid-chunk-size"));
                }
                carry_len += n;
            };
            let size_field = core::str::from_utf8(&resp_buf[..line_end]).map_err(|_| String::from("chunk size line isn't valid UTF-8"))?;
            let size_field = size_field.split(';').next().unwrap_or(""); // drop chunk extensions, if any
            let size =
                usize::from_str_radix(size_field.trim(), 16).map_err(|_| format!("bad chunk size {size_field:?}"))?;

            let after_line = line_end + 2; // the chunk-size line's own trailing CRLF
            resp_buf.copy_within(after_line..carry_len, 0);
            carry_len -= after_line;

            if size == 0 {
                // Final chunk: consume the trailer section (usually just
                // one more CRLF, but RFC 7230 allows trailer headers) up
                // to its terminating blank line, then this response is
                // fully drained.
                loop {
                    if resp_buf[..carry_len].windows(4).position(|w| w == b"\r\n\r\n").is_some()
                        || (carry_len >= 2 && &resp_buf[..2] == b"\r\n")
                    {
                        break;
                    }
                    if carry_len >= resp_buf.len() {
                        return Err(String::from("chunked trailer too long for the scratch buffer"));
                    }
                    let n = session.read(&mut resp_buf[carry_len..]).await.map_err(|e| format!("trailer read failed: {e}"))?;
                    if n == 0 {
                        return Err(String::from("connection closed mid-trailer"));
                    }
                    carry_len += n;
                }
                break;
            }

            let mut remaining = size + 2; // chunk data plus its own trailing CRLF
            let take = remaining.min(carry_len);
            resp_buf.copy_within(take..carry_len, 0);
            carry_len -= take;
            remaining -= take;
            while remaining > 0 {
                let to_read = remaining.min(discard.len());
                let n =
                    session.read(&mut discard[..to_read]).await.map_err(|e| format!("chunk data read failed: {e}"))?;
                if n == 0 {
                    return Err(String::from("connection closed mid-chunk-data"));
                }
                remaining -= n;
            }
        }
    } else {
        // Neither `Content-Length` nor `Transfer-Encoding: chunked`: this
        // response's body (if any) is only delimited by the connection
        // closing, which this side can't safely wait for without breaking
        // its own `PERIOD` cadence. Not an error -- just not safe to keep
        // this connection open for another request.
        keep_alive = false;
    }

    Ok((status, keep_alive))
}
