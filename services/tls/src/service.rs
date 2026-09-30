//! ESP crypto and Embassy socket adapter for the portable IOBEWI TLS service.

use embassy_net::{Stack, tcp::TcpSocket};
use embedded_io_async::{ErrorType, Read, Write};
use iobewi_config_space::ConfigSpace;
use iobewi_tls::{Identity, PairError, TlsCrypto, TlsService};
use iobewi_transport::{Close, SecureClientTransport};
use log::warn;
use mbedtls_rs::{Session, SessionConfig, SessionError};

pub use iobewi_config_space::ConfigBackend;
pub use iobewi_tls::{CONFIG_BUDGET, IdentityBootstrapError, SaveCertError};
pub use crate::TlsReferenceStatic;

pub type TlsConfigSpace<B> = ConfigSpace<B>;

/// Connected client transport. Protocol callers depend on the async I/O
/// traits; the concrete MbedTLS session stays in the ESP adapter.
///
/// A newtype rather than a type alias over `Session` -- `iobewi_transport`'s
/// `Close` is a foreign trait and `Session` a foreign type, so implementing
/// one for the other here needs a locally-owned wrapper (Rust's orphan
/// rule). Read/Write are delegated straight through to the inner session.
pub struct ClientStream<'buf>(Session<'static, TcpSocket<'buf>>);

impl ErrorType for ClientStream<'_> {
    type Error = SessionError;
}

impl Read for ClientStream<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, SessionError> {
        self.0.read(buf).await
    }
}

impl Write for ClientStream<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, SessionError> {
        self.0.write(buf).await
    }

    async fn flush(&mut self) -> Result<(), SessionError> {
        self.0.flush().await
    }
}

impl Close for ClientStream<'_> {
    type Error = SessionError;

    async fn close(&mut self) -> Result<(), SessionError> {
        self.0.close().await
    }
}

struct EspCrypto;

impl TlsCrypto for EspCrypto {
    type ServerConfig = SessionConfig<'static>;

    fn validate_pair(&self, cert: &str, key: &str) -> Result<(), PairError> {
        crate::validate_cert_key_pair(cert, key).map_err(|e| match e {
            crate::PairError::Mismatch => PairError::Mismatch,
            crate::PairError::InvalidCertificate | crate::PairError::InvalidPrivateKey => PairError::Invalid,
        })
    }

    fn server_config(&self, cert: &str, key: &str) -> Option<Self::ServerConfig> {
        crate::server_config_from_pem(cert, key).ok()
    }

    fn generate_identity(&self, common_name: &str) -> Option<Identity> {
        let generated = crate::generate_self_signed_identity(common_name).map_err(|e| {
            warn!("tls: identity generation failed: {e:?}");
        }).ok()?;
        Some(Identity { cert_pem: generated.cert_pem, key_pem: generated.key_pem })
    }

    fn validate_ca(&self, ca: &str) -> bool {
        crate::validate_ca_pem(ca)
    }
}

pub fn init(now: crate::UnixTimeFn) -> TlsReferenceStatic {
    crate::init(now)
}

pub async fn ensure_server_identity<B: ConfigBackend>(space: &TlsConfigSpace<B>, common_name: &str) -> Result<(), IdentityBootstrapError>
where B::Error: core::fmt::Debug {
    TlsService::new(EspCrypto).ensure_server_identity(space, common_name).await
}

pub async fn server_identity_valid<B: ConfigBackend>(space: &TlsConfigSpace<B>) -> bool
where B::Error: core::fmt::Debug {
    TlsService::new(EspCrypto).server_identity_valid(space).await
}

pub async fn save_cert<B: ConfigBackend>(space: &TlsConfigSpace<B>, cert_pem: &str, key_pem: &str) -> Result<(), SaveCertError>
where B::Error: core::fmt::Debug {
    TlsService::new(EspCrypto).save_cert(space, cert_pem, key_pem).await
}

pub async fn server_config<B: ConfigBackend>(space: &TlsConfigSpace<B>) -> Option<SessionConfig<'static>>
where B::Error: core::fmt::Debug {
    TlsService::new(EspCrypto).server_config(space).await
}

pub async fn save_ca<B: ConfigBackend>(space: &TlsConfigSpace<B>, ca_pem: &str) -> Result<(), SaveCertError>
where B::Error: core::fmt::Debug {
    TlsService::new(EspCrypto).save_ca(space, ca_pem).await
}

pub enum ClientTlsError {
    ClockUnsynced,
    NoCa,
    BadCa,
    /// `host` contains an embedded NUL byte.
    InvalidHost,
    Dns,
    Tcp(embassy_net::tcp::ConnectError),
    Handshake(SessionError),
}

impl core::fmt::Display for ClientTlsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ClockUnsynced => write!(f, "clock not synchronized yet (SNTP)"),
            Self::NoCa => write!(f, "no CA configured (POST /v1alpha1/tls/ca)"),
            Self::BadCa => write!(f, "stored CA failed to parse"),
            Self::InvalidHost => write!(f, "host name contains a nul byte"),
            Self::Dns => write!(f, "DNS resolution failed"),
            Self::Tcp(e) => write!(f, "TCP connect failed: {e:?}"),
            Self::Handshake(e) => write!(f, "TLS handshake failed: {e}"),
        }
    }
}

/// Connect only when the application wall clock has synchronized and the
/// durable CA is present. MbedTLS and Embassy network details stay here.
pub async fn connect_client<'buf, B: ConfigBackend>(
    tls: TlsReferenceStatic,
    stack: Stack<'static>,
    space: &TlsConfigSpace<B>,
    clock_is_set: bool,
    rx_buffer: &'buf mut [u8],
    tx_buffer: &'buf mut [u8],
    host: &str,
    port: u16,
) -> Result<ClientStream<'buf>, ClientTlsError>
where B::Error: core::fmt::Debug {
    if !clock_is_set { return Err(ClientTlsError::ClockUnsynced); }
    let ca = TlsService::new(EspCrypto).trusted_ca(space).await.ok_or(ClientTlsError::NoCa)?;
    crate::embassy::connect_client(tls, stack, rx_buffer, tx_buffer, host, port, &ca)
        .await
        .map(ClientStream)
        .map_err(|e| match e {
            crate::embassy::ClientConnectError::BadCa => ClientTlsError::BadCa,
            crate::embassy::ClientConnectError::InvalidHost => ClientTlsError::InvalidHost,
            crate::embassy::ClientConnectError::Dns => ClientTlsError::Dns,
            crate::embassy::ClientConnectError::Tcp(e) => ClientTlsError::Tcp(e),
            crate::embassy::ClientConnectError::Handshake(e) => ClientTlsError::Handshake(e),
        })
}

/// ESP implementation of the portable `SecureClientTransport` capability.
/// Trust material (the TLS config space holding the CA, and whether the
/// clock has converged) is bound here, at construction -- `connect()`
/// itself takes only `host`/`port`/buffers, so every call automatically
/// gets the same fail-closed policy without the caller having to know it
/// exists.
#[derive(Clone, Copy)]
pub struct EspClientTransport<B: ConfigBackend + 'static> {
    pub tls: TlsReferenceStatic,
    pub stack: Stack<'static>,
    pub tls_config: &'static TlsConfigSpace<B>,
    pub clock_is_set: fn() -> bool,
}

impl<B> SecureClientTransport for EspClientTransport<B>
where
    B: ConfigBackend + 'static,
    B::Error: core::fmt::Debug,
{
    type Error = ClientTlsError;
    type Connection<'a>
        = ClientStream<'a>
    where
        Self: 'a;

    async fn connect<'a>(
        &'a self,
        host: &'a str,
        port: u16,
        rx: &'a mut [u8],
        tx: &'a mut [u8],
    ) -> Result<Self::Connection<'a>, Self::Error> {
        connect_client(self.tls, self.stack, self.tls_config, (self.clock_is_set)(), rx, tx, host, port).await
    }

    fn local_address(&self) -> Option<alloc::string::String> {
        self.stack.config_v4().map(|c| alloc::format!("{}", c.address.address()))
    }
}
