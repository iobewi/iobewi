#![no_std]

//! HTTPS entry point: only a completed TLS handshake yields an HTTP socket.

use picoserve::io::Socket;
use picoserve::routing::PathRouter;
use picoserve::{EmbassyRuntime, Router};

/// A listener that only returns sockets after a successful TLS handshake.
/// Missing identity or handshake failure must return an error, never TCP.
#[allow(async_fn_in_trait)]
pub trait TlsListener {
    type Connection<'a>: Socket<EmbassyRuntime>
    where
        Self: 'a;

    async fn accept_tls(&mut self) -> Result<Self::Connection<'_>, ()>;
}

/// Run the portable HTTP dispatcher using TLS-only connections: accept a
/// completed handshake, serve it, repeat (same loop and buffer as the generic
/// `iobewi_http_server::serve_forever_io`, over native picoserve sockets).
pub async fn serve_forever<L: TlsListener, R: PathRouter>(
    listener: &mut L,
    router: &Router<R>,
) -> ! {
    let config = iobewi_http_server::server_config();
    let mut http_buffer = [0u8; iobewi_http_server::HTTP_BUFFER_LEN];
    loop {
        match listener.accept_tls().await {
            Ok(socket) => iobewi_http_server::serve_one(router, &config, &mut http_buffer, socket).await,
            Err(()) => {
                // The listener owns the retry delay and any platform log.
            }
        }
    }
}
