#![cfg_attr(not(test), no_std)]
#![allow(async_fn_in_trait)]

//! Portable client-side secure transport capability.
//!
//! [`SecureClientTransport`] lets a caller ask for a connection to
//! `host:port` and get back an authenticated, encrypted byte stream, without
//! ever knowing how DNS, TCP, the TLS handshake, certificates, the clock, or
//! the underlying network stack are implemented. Those are the platform
//! implementation's own construction-time concern -- trust material (CA,
//! clock) is injected when the implementation is built, never passed to
//! `connect()` itself, so the same fail-closed policy applies to every call
//! without the caller having to know it exists.
//!
//! This crate deliberately does not cover:
//! - the TLS *server* side (accepting connections) -- see `iobewi-https`'s
//!   `TlsListener`, a separate, already-existing capability;
//! - identity/CA/crypto policy -- see `iobewi-tls`'s `TlsCrypto`;
//! - entropy (random bytes/u32) -- not a property of a secure transport;
//!   a consumer that needs it (e.g. WebSocket frame masking) should ask for
//!   it as its own, narrower capability.

use embedded_io_async::{Read, Write};

/// A connection that can be shut down cleanly (e.g. a TLS close-notify)
/// instead of an abrupt reset. Kept separate from `Read`/`Write` because not
/// every consumer needs it -- a client that just drops the connection on
/// reconnect (rather than closing it) has no use for this.
pub trait Close {
    type Error;

    async fn close(&mut self) -> Result<(), Self::Error>;
}

/// Client-side secure transport. An implementation owns DNS, TCP, the
/// handshake, and its own trust policy (CA, clock, crypto backend); the
/// caller only ever sees `Ok(Connection)` or `Err(Error)`.
pub trait SecureClientTransport {
    /// Failure connecting (DNS, TCP, handshake, trust policy refusing the
    /// attempt). Meant to be logged (`Display`), not matched on across a
    /// crate boundary -- the whole point is that callers don't need to know
    /// which platform-specific step failed.
    type Error: core::fmt::Display;

    type Connection<'a>: Read + Write + Close
    where
        Self: 'a;

    /// Connects to `host:port` and returns an authenticated, encrypted
    /// stream. `rx`/`tx` back the connection's own transport-layer buffers
    /// for as long as the connection is used.
    async fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
        rx: &'a mut [u8],
        tx: &'a mut [u8],
    ) -> Result<Self::Connection<'a>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;
    use core::convert::Infallible;
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::collections::VecDeque;

    fn block_on<F: Future>(mut future: F) -> F::Output {
        unsafe fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(core::ptr::null(), &VTABLE)
        }
        unsafe fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);

        let raw = RawWaker::new(core::ptr::null(), &VTABLE);
        let waker = unsafe { Waker::from_raw(raw) };
        let mut cx = Context::from_waker(&waker);
        let mut future = unsafe { Pin::new_unchecked(&mut future) };

        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    /// In-memory loopback connection: whatever is written is what the next
    /// read returns, byte for byte. Enough to exercise the trait's shape
    /// without a real network stack.
    #[derive(Debug)]
    struct Loopback(RefCell<VecDeque<u8>>);

    impl embedded_io_async::ErrorType for Loopback {
        type Error = Infallible;
    }

    impl Read for Loopback {
        async fn read(&mut self, out: &mut [u8]) -> Result<usize, Infallible> {
            let mut buf = self.0.borrow_mut();
            let n = out.len().min(buf.len());
            for slot in out.iter_mut().take(n) {
                *slot = buf.pop_front().expect("checked by min() above");
            }
            Ok(n)
        }
    }

    impl Write for Loopback {
        async fn write(&mut self, data: &[u8]) -> Result<usize, Infallible> {
            self.0.borrow_mut().extend(data.iter().copied());
            Ok(data.len())
        }

        async fn flush(&mut self) -> Result<(), Infallible> {
            Ok(())
        }
    }

    impl Close for Loopback {
        type Error = Infallible;

        async fn close(&mut self) -> Result<(), Infallible> {
            Ok(())
        }
    }

    struct AlwaysConnects;

    impl SecureClientTransport for AlwaysConnects {
        type Error = &'static str;
        type Connection<'a>
            = Loopback
        where
            Self: 'a;

        async fn connect<'a>(
            &'a self,
            host: &'a str,
            _port: u16,
            _rx: &'a mut [u8],
            _tx: &'a mut [u8],
        ) -> Result<Self::Connection<'a>, Self::Error> {
            if host.is_empty() {
                return Err("empty host refused");
            }
            Ok(Loopback(RefCell::new(VecDeque::new())))
        }
    }

    #[test]
    fn connect_then_round_trip_and_close() {
        let transport = AlwaysConnects;
        let mut rx = [0u8; 16];
        let mut tx = [0u8; 16];
        let mut conn = block_on(transport.connect("example.test", 443, &mut rx, &mut tx)).unwrap();

        block_on(conn.write(b"ping")).unwrap();
        let mut out = [0u8; 4];
        block_on(conn.read(&mut out)).unwrap();
        assert_eq!(&out, b"ping");
        block_on(conn.close()).unwrap();
    }

    #[test]
    fn connect_reports_the_implementation_error_without_a_connection() {
        let transport = AlwaysConnects;
        let mut rx = [0u8; 16];
        let mut tx = [0u8; 16];
        let err = block_on(transport.connect("", 443, &mut rx, &mut tx)).unwrap_err();
        assert_eq!(err, "empty host refused");
    }
}
