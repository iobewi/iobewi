#![no_std]

//! HTTP request handling for any connected socket. The caller chooses the
//! transport and supplies routes; this crate never opens a network port.

extern crate alloc;

use picoserve::io::Socket;
use picoserve::routing::PathRouter;
use picoserve::{Config, EmbassyRuntime, Router};

/// Shared HTTP routing and response types for framework services. A service
/// contributes handlers to the caller's router; the application selects the
/// complete route set and the transport listener.
pub use picoserve::{io, request, response, routing, ResponseSent};
pub use picoserve::Router as HttpRouter;

pub mod json;
pub mod range;

/// Platform capability: accept a connection over the caller-selected transport.
/// TLS enforcement belongs to `iobewi-https`.
#[allow(async_fn_in_trait)]
pub trait ConnectionListener {
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
pub async fn serve_forever<L: ConnectionListener, R: PathRouter>(
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
