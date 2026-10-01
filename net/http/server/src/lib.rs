#![no_std]

//! HTTP request handling for any connected socket. The caller chooses the
//! transport and supplies routes; this crate never opens a network port.

extern crate alloc;

use iobewi_net_io::ConnectionListener;
use picoserve::io::Socket;
use picoserve::routing::PathRouter;
use picoserve::{Config, EmbassyRuntime, Router};

/// Shared HTTP routing and response types for framework services. A service
/// contributes handlers to the caller's router; the application selects the
/// complete route set and the transport listener.
pub use picoserve::{io, request, response, routing, ResponseSent};
pub use picoserve::Router as HttpRouter;

pub mod auth;
pub mod io_socket;
pub mod client;
pub mod json;
pub mod range;
pub mod stream;
pub mod websocket;

/// Accepts connections that are already `picoserve` sockets.
///
/// TEMPORARY (S3 migration debt): this is the former `ConnectionListener`
/// contract, kept only for listeners whose connections are native picoserve
/// sockets (the TLS path: `iobewi-https`'s `TlsListener`). The generic,
/// protocol-free contract is `iobewi_net_io::ConnectionListener`, served
/// through [`serve_forever_io`] and the [`io_socket::IoSocket`] adapter.
/// TLS enforcement belongs to `iobewi-https`.
#[allow(async_fn_in_trait)]
pub trait SocketListener {
    type Connection<'a>: Socket<EmbassyRuntime>
    where
        Self: 'a;

    async fn accept(&mut self) -> Result<Self::Connection<'_>, ()>;
}

/// Serve a connected socket with a caller-provided router.
/// Services and applications may contribute routes to the same router.
pub async fn serve_connection<R: PathRouter, S: Socket<EmbassyRuntime>>(
    router: &Router<R>,
    config: &Config,
    http_buffer: &mut [u8],
    socket: S,
) -> Result<picoserve::DisconnectionInfo<picoserve::NoGracefulShutdown>, picoserve::Error<S::Error>> {
    picoserve::Server::new(router, config, http_buffer).serve(socket).await
}

/// Serve one connection at a time, reusing the HTTP buffer. The listener
/// determines how connections are accepted and whether they are encrypted.
pub async fn serve_forever<L: SocketListener, R: PathRouter>(
    listener: &mut L,
    router: &Router<R>,
) -> ! {
    let config = Config::const_default().keep_connection_alive();
    let mut http_buffer = [0u8; 2048];
    loop {
        match listener.accept().await {
            Ok(socket) => {
                if serve_connection(router, &config, &mut http_buffer, socket).await.is_err() {
                    log::debug!("http: connection closed with an error");
                }
            }
            Err(()) => {
                // The listener owns the retry delay and any platform log.
            }
        }
    }
}

struct IoListener<'l, L>(&'l mut L);

impl<L: ConnectionListener> SocketListener for IoListener<'_, L> {
    type Connection<'a> = io_socket::IoSocket<L::Connection<'a>> where Self: 'a;

    async fn accept(&mut self) -> Result<Self::Connection<'_>, ()> {
        self.0.accept().await.map(io_socket::IoSocket::new)
    }
}

/// Serve one connection at a time on a protocol-free `net/io` listener,
/// adapting each accepted connection to a picoserve socket.
pub async fn serve_forever_io<L: ConnectionListener, R: PathRouter>(
    listener: &mut L,
    router: &Router<R>,
) -> ! {
    serve_forever(&mut IoListener(listener), router).await
}
