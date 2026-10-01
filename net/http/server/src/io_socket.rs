//! Adapter: any `net/io` connection as a picoserve socket.
//!
//! picoserve wants to split a socket into concurrently usable read and write
//! halves; a generic `Read + Write` stream cannot be split, so both halves
//! share the connection behind an async mutex (the same technique the TLS
//! session adapter uses). `shutdown` is the connection's `Close`; `abort`
//! drops it.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use iobewi_net_io::{Connection, ErrorType, Read, Write};
use picoserve::mem::BorrowedBuffer;
use picoserve::time::Timer;
use picoserve::{EmbassyRuntime, Timeouts};

/// The I/O error of an [`IoSocket`]: the `kind` of the underlying
/// connection's error. picoserve needs a `'static` socket error; a generic
/// connection's own error type may borrow, so only its kind crosses over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoError(pub embedded_io_async::ErrorKind);

impl core::fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "connection I/O error: {:?}", self.0)
    }
}

impl core::error::Error for IoError {}

impl embedded_io_async::Error for IoError {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        self.0
    }
}

fn io_error<E: embedded_io_async::Error>(error: E) -> IoError {
    IoError(error.kind())
}

/// A `net/io` connection presented as a picoserve socket.
pub struct IoSocket<C> {
    connection: Mutex<CriticalSectionRawMutex, C>,
}

impl<C> IoSocket<C> {
    pub fn new(connection: C) -> Self {
        Self { connection: Mutex::new(connection) }
    }
}

/// One half (read or write) of an [`IoSocket`].
pub struct IoHalf<'a, C> {
    connection: &'a Mutex<CriticalSectionRawMutex, C>,
}

impl<C> ErrorType for IoHalf<'_, C> {
    type Error = IoError;
}

impl<C: Read> Read for IoHalf<'_, C> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.connection.lock().await.read(buf).await.map_err(io_error)
    }
}

impl<C: Write> Write for IoHalf<'_, C> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.connection.lock().await.write(buf).await.map_err(io_error)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.connection.lock().await.flush().await.map_err(io_error)
    }
}

impl<C: Write> picoserve::io::Write for IoHalf<'_, C> {
    async fn write_with<F: FnOnce(picoserve::mem::BorrowedCursor<'_>) -> R, R>(
        &mut self,
        f: F,
    ) -> Result<R, Self::Error> {
        let mut buffer = [0u8; 1024];
        let mut buffer = BorrowedBuffer::new(&mut buffer);
        let output = f(buffer.unfilled());
        self.connection.lock().await.write_all(buffer.filled()).await.map_err(io_error)?;
        Ok(output)
    }
}

impl<C: Connection> picoserve::io::Socket<EmbassyRuntime> for IoSocket<C> {
    type Error = IoError;
    type ReadHalf<'b>
        = IoHalf<'b, C>
    where
        Self: 'b;
    type WriteHalf<'b>
        = IoHalf<'b, C>
    where
        Self: 'b;

    fn split(&mut self) -> (Self::ReadHalf<'_>, Self::WriteHalf<'_>) {
        (IoHalf { connection: &self.connection }, IoHalf { connection: &self.connection })
    }

    async fn abort<T: Timer<EmbassyRuntime>>(
        self,
        _timeouts: &Timeouts,
        _timer: &T,
    ) -> Result<(), picoserve::Error<Self::Error>> {
        // Dropping the connection is the only abort a generic stream has.
        Ok(())
    }

    async fn shutdown<T: Timer<EmbassyRuntime>>(
        self,
        _timeouts: &Timeouts,
        _timer: &T,
    ) -> Result<(), picoserve::Error<Self::Error>> {
        self.connection.into_inner().close().await.map_err(|e| picoserve::Error::Write(io_error(e)))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use iobewi_net_io::Close;
    use alloc::collections::VecDeque;
    use core::convert::Infallible;
    use core::future::Future;
    use core::task::{Context, Poll, Waker};
    use picoserve::io::Socket;

    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = core::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("loopback futures are always ready"),
        }
    }

    #[derive(Default)]
    struct Loopback {
        bytes: VecDeque<u8>,
        closed: bool,
    }

    impl ErrorType for Loopback {
        type Error = Infallible;
    }

    impl Read for Loopback {
        async fn read(&mut self, out: &mut [u8]) -> Result<usize, Infallible> {
            let n = out.len().min(self.bytes.len());
            for slot in out.iter_mut().take(n) {
                *slot = self.bytes.pop_front().unwrap();
            }
            Ok(n)
        }
    }

    impl Write for Loopback {
        async fn write(&mut self, data: &[u8]) -> Result<usize, Infallible> {
            self.bytes.extend(data.iter().copied());
            Ok(data.len())
        }

        async fn flush(&mut self) -> Result<(), Infallible> {
            Ok(())
        }
    }

    impl Close for Loopback {
        async fn close(&mut self) -> Result<(), Infallible> {
            self.closed = true;
            Ok(())
        }
    }

    #[test]
    fn halves_share_the_connection() {
        let mut socket = IoSocket::new(Loopback::default());
        let (mut rx, mut tx) = socket.split();
        ready(picoserve::io::Write::write_with(&mut tx, |mut cursor| {
            cursor.try_append(b"hello").unwrap();
        }))
        .unwrap();
        let mut out = [0u8; 5];
        assert_eq!(ready(rx.read(&mut out)).unwrap(), 5);
        assert_eq!(&out, b"hello");
    }
}
