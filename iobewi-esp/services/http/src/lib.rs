#![no_std]

//! ESP TCP listener. The application decides whether to expose plaintext
//! HTTP; administrative routes are wired through `iobewi-esp-https` instead.

use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use embedded_io_async::{ErrorType, Read, Write};
use iobewi_net_io::{Close, ConnectionListener};
use log::warn;
use picoserve::routing::PathRouter;

/// ESP TCP capability used by both HTTP and the HTTPS handshake adapter.
pub struct EspTcpListener<'a> {
    stack: Stack<'static>,
    port: u16,
    rx: &'a mut [u8],
    tx: &'a mut [u8],
}

impl<'a> EspTcpListener<'a> {
    pub fn new(stack: Stack<'static>, port: u16, rx: &'a mut [u8], tx: &'a mut [u8]) -> Self {
        Self { stack, port, rx, tx }
    }

    pub async fn accept_connection(&mut self) -> Result<TcpSocket<'_>, ()> {
        let mut socket = TcpSocket::new(self.stack, &mut *self.rx, &mut *self.tx);
        if let Err(e) = socket.accept(self.port).await {
            warn!("TCP: accept failed: {e:?}");
            return Err(());
        }
        Ok(socket)
    }
}

/// An accepted ESP TCP connection as a `net/io` connection: reads and writes
/// go straight to the embassy-net socket; `close` is a graceful TCP close
/// (FIN) followed by a flush.
pub struct EspTcpStream<'a>(TcpSocket<'a>);

impl ErrorType for EspTcpStream<'_> {
    type Error = embassy_net::tcp::Error;
}

impl Read for EspTcpStream<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.read(buf).await
    }
}

impl Write for EspTcpStream<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.0.write(buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.flush().await
    }
}

impl Close for EspTcpStream<'_> {
    async fn close(&mut self) -> Result<(), Self::Error> {
        self.0.close();
        self.0.flush().await
    }
}

impl ConnectionListener for EspTcpListener<'_> {
    type Connection<'a> = EspTcpStream<'a> where Self: 'a;

    async fn accept(&mut self) -> Result<Self::Connection<'_>, ()> {
        self.accept_connection().await.map(EspTcpStream)
    }
}

/// Serve HTTP on an explicitly selected TCP port. Only applications that
/// opt into this function open a plaintext listener.
pub async fn serve<R: PathRouter>(
    stack: Stack<'static>,
    port: u16,
    router: &picoserve::Router<R>,
) -> ! {
    let mut rx = [0u8; 1024];
    let mut tx = [0u8; 1024];
    let mut listener = EspTcpListener::new(stack, port, &mut rx, &mut tx);
    iobewi_http::serve_forever_io(&mut listener, router).await
}
