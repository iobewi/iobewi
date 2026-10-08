#![no_std]

//! TCP transport over an `embassy-net` stack: a listener and its accepted socket as
//! `net/io` connections. Knows no HTTP framework, no TLS and no hardware (it works on any
//! platform that provides an `embassy-net` stack); the TLS listener
//! (`iobewi-esp-tls`) and the HTTP server (`iobewi-http-server`) are layered
//! on top by the composition root. There is deliberately no plaintext HTTP
//! entry point here: the agent exposes administrative routes over TLS only,
//! and port 80 stays closed.

use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use embedded_io_async::{ErrorType, Read, Write};
use iobewi_net_io::{Close, ConnectionListener};
use log::warn;

/// TCP listener on an `embassy-net` stack, the base of the TLS listener.
pub struct TcpListener<'a> {
    stack: Stack<'static>,
    port: u16,
    rx: &'a mut [u8],
    tx: &'a mut [u8],
}

impl<'a> TcpListener<'a> {
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

/// An accepted TCP connection as a `net/io` connection: reads and writes
/// go straight to the embassy-net socket; `close` is a graceful TCP close
/// (FIN) followed by a flush.
pub struct TcpStream<'a>(TcpSocket<'a>);

impl ErrorType for TcpStream<'_> {
    type Error = embassy_net::tcp::Error;
}

impl Read for TcpStream<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.read(buf).await
    }
}

impl Write for TcpStream<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.0.write(buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.0.flush().await
    }
}

impl Close for TcpStream<'_> {
    async fn close(&mut self) -> Result<(), Self::Error> {
        self.0.close();
        self.0.flush().await
    }
}

impl ConnectionListener for TcpListener<'_> {
    type Connection<'a> = TcpStream<'a> where Self: 'a;

    async fn accept(&mut self) -> Result<Self::Connection<'_>, ()> {
        self.accept_connection().await.map(TcpStream)
    }
}
