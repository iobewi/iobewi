#![no_std]

//! ESP TCP listener. The application decides whether to expose plaintext
//! HTTP; administrative routes are wired through `iobewi-esp-https` instead.

use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use iobewi_http::ConnectionListener;
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

impl ConnectionListener for EspTcpListener<'_> {
    type Connection<'a> = TcpSocket<'a> where Self: 'a;

    async fn accept(&mut self) -> Result<Self::Connection<'_>, ()> {
        self.accept_connection().await
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
    iobewi_http::serve_forever(&mut listener, router).await
}
