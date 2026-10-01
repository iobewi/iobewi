#![no_std]

//! HTTPS entry point: only a completed TLS handshake yields an HTTP socket.

use iobewi_http_server::SocketListener;
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

struct TlsConnections<'a, L>(&'a mut L);

impl<L: TlsListener> SocketListener for TlsConnections<'_, L> {
    type Connection<'a> = L::Connection<'a> where Self: 'a;

    async fn accept(&mut self) -> Result<Self::Connection<'_>, ()> {
        self.0.accept_tls().await
    }
}

/// Run the portable HTTP dispatcher using TLS-only connections.
pub async fn serve_forever<L: TlsListener, R: PathRouter>(
    listener: &mut L,
    router: &Router<R>,
) -> ! {
    iobewi_http_server::serve_forever(&mut TlsConnections(listener), router).await
}
