#![no_std]

//! ESP32-S3 TLS listener for the portable IOBEWI HTTP server.

use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use embassy_time::{Duration, Timer, with_timeout};
use iobewi_esp_tls::mbedtls_rs::{Session, SessionConfig, SessionError};
use iobewi_esp_tls::{TlsReferenceStatic, embassy::PicoserveTlsSocket};
use iobewi_http::TlsListener;
use log::{debug, warn};
use picoserve::routing::PathRouter;

const ADMIN_PORT_HTTPS: u16 = 443;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

struct EspTlsListener<'a, LoadIdentity> {
    stack: Stack<'static>,
    tls: TlsReferenceStatic,
    identity: LoadIdentity,
    config: Option<SessionConfig<'static>>,
    rx: &'a mut [u8],
    tx: &'a mut [u8],
}

impl<LoadIdentity> TlsListener for EspTlsListener<'_, LoadIdentity>
where
    LoadIdentity: AsyncFn() -> Option<SessionConfig<'static>>,
{
    type Connection<'a> = PicoserveTlsSocket<'a, 'a> where Self: 'a;

    async fn accept_tls(&mut self) -> Result<Self::Connection<'_>, ()> {
        let Some(config) = (self.identity)().await else {
            warn!("HTTPS: no usable server identity; administrative surface remains closed");
            Timer::after(Duration::from_secs(5)).await;
            return Err(());
        };
        self.config = Some(config);

        let mut socket = TcpSocket::new(self.stack, &mut *self.rx, &mut *self.tx);
        if let Err(e) = socket.accept(ADMIN_PORT_HTTPS).await {
            warn!("HTTPS: accept failed: {e:?}");
            return Err(());
        }
        socket.set_keep_alive(Some(Duration::from_secs(30)));
        socket.set_timeout(Some(Duration::from_secs(45)));

        let Some(server_config) = self.config.as_ref() else {
            return Err(());
        };
        let mut session = match Session::new(self.tls, socket, server_config) {
            Ok(session) => session,
            Err(e) => {
                warn!("HTTPS: session setup failed: {e}");
                return Err(());
            }
        };
        match with_timeout(HANDSHAKE_TIMEOUT, session.connect()).await {
            Ok(Ok(())) => Ok(PicoserveTlsSocket::new(session)),
            Ok(Err(e)) if is_peer_hangup(&e) => {
                debug!("HTTPS: handshake aborted by peer: {e}");
                Err(())
            }
            Ok(Err(e)) => {
                warn!("HTTPS: handshake failed: {e}");
                Err(())
            }
            Err(_) => {
                warn!("HTTPS: handshake timed out after {HANDSHAKE_TIMEOUT:?}");
                Err(())
            }
        }
    }
}

fn is_peer_hangup(e: &SessionError) -> bool {
    const MBEDTLS_ERR_SSL_FATAL_ALERT_MESSAGE: i32 = -0x7780;
    matches!(e, SessionError::MbedTls(m) if m.code() == MBEDTLS_ERR_SSL_FATAL_ALERT_MESSAGE)
}

/// ESP adapter for IOBEWI's shared HTTP server. `identity` reads the current
/// server certificate and key through an application-provided capability.
/// A missing identity never falls back to unencrypted HTTP.
pub async fn serve<R, LoadIdentity>(
    stack: Stack<'static>,
    tls: TlsReferenceStatic,
    identity: LoadIdentity,
    router: &picoserve::Router<R>,
) -> !
where
    R: PathRouter,
    LoadIdentity: AsyncFn() -> Option<SessionConfig<'static>>,
{
    let mut rx = [0u8; 1024];
    let mut tx = [0u8; 1024];
    let mut listener = EspTlsListener {
        stack,
        tls,
        identity,
        config: None,
        rx: &mut rx,
        tx: &mut tx,
    };
    iobewi_http::serve_forever(&mut listener, router).await
}
