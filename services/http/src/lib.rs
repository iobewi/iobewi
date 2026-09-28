#![no_std]

//! Shared HTTP server. The platform adapter supplies a *handshaken* TLS
//! socket; this module never accepts raw TCP or provides a cleartext fallback.

use picoserve::io::Socket;
use picoserve::routing::PathRouter;
use picoserve::{Config, EmbassyRuntime, Router};

/// Platform capability: accept a connection only after TLS has completed.
/// Return an error if the server identity is missing or the handshake fails.
/// In particular, implementations must never substitute a plain TCP socket.
#[allow(async_fn_in_trait)]
pub trait TlsListener {
    type Connection<'a>: Socket<EmbassyRuntime>
    where
        Self: 'a;

    async fn accept_tls(&mut self) -> Result<Self::Connection<'_>, ()>;
}

/// Serve an already authenticated TLS socket with a caller-provided router.
/// Services and applications may contribute routes to the same router.
pub async fn serve_connection<R: PathRouter, S: Socket<EmbassyRuntime>>(
    router: &Router<R>,
    config: &Config,
    http_buffer: &mut [u8],
    socket: S,
) -> Result<picoserve::DisconnectionInfo<picoserve::NoGracefulShutdown>, picoserve::Error<S::Error>> {
    picoserve::Server::new(router, config, http_buffer).serve(socket).await
}

/// Serve one TLS connection at a time, reusing the HTTP buffer. A failed
/// accept (including missing identity) does not open any alternate listener.
/// The platform should delay/retry transient errors inside `accept_tls`.
pub async fn serve_forever<L: TlsListener, R: PathRouter>(
    listener: &mut L,
    router: &Router<R>,
) -> ! {
    let config = Config::const_default().keep_connection_alive();
    let mut http_buffer = [0u8; 2048];
    loop {
        match listener.accept_tls().await {
            Ok(socket) => {
                if serve_connection(router, &config, &mut http_buffer, socket).await.is_err() {
                    log::debug!("http: TLS connection closed with an error");
                }
            }
            Err(()) => {
                // The listener owns the retry delay and any platform log.
            }
        }
    }
}
