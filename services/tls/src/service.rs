//! ESP crypto and Embassy socket adapter for the portable IOBEWI TLS service.

use embassy_net::{Stack, tcp::TcpSocket};
use iobewi_config_space::ConfigSpace;
use iobewi_esp_config_space::NvsConfigBackend;
use iobewi_tls::{Identity, PairError, TlsCrypto, TlsService};
use log::warn;
use mbedtls_rs::{Session, SessionConfig, SessionError};

pub use iobewi_tls::{CONFIG_BUDGET, IdentityBootstrapError, SaveCertError};
pub use crate::TlsReferenceStatic;

pub type TlsConfigSpace = ConfigSpace<NvsConfigBackend>;

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

pub async fn ensure_server_identity(space: &TlsConfigSpace, common_name: &str) -> Result<(), IdentityBootstrapError> {
    TlsService::new(EspCrypto).ensure_server_identity(space, common_name).await
}

pub async fn server_identity_valid(space: &TlsConfigSpace) -> bool {
    TlsService::new(EspCrypto).server_identity_valid(space).await
}

pub async fn save_cert(space: &TlsConfigSpace, cert_pem: &str, key_pem: &str) -> Result<(), SaveCertError> {
    TlsService::new(EspCrypto).save_cert(space, cert_pem, key_pem).await
}

pub async fn server_config(space: &TlsConfigSpace) -> Option<SessionConfig<'static>> {
    TlsService::new(EspCrypto).server_config(space).await
}

pub async fn save_ca(space: &TlsConfigSpace, ca_pem: &str) -> Result<(), SaveCertError> {
    TlsService::new(EspCrypto).save_ca(space, ca_pem).await
}

pub enum ClientTlsError {
    ClockUnsynced,
    NoCa,
    BadCa,
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
            Self::Dns => write!(f, "DNS resolution failed"),
            Self::Tcp(e) => write!(f, "TCP connect failed: {e:?}"),
            Self::Handshake(e) => write!(f, "TLS handshake failed: {e}"),
        }
    }
}

/// Connect only when the application wall clock has synchronized and the
/// durable CA is present. MbedTLS and Embassy network details stay here.
pub async fn connect_client<'h, 'buf>(
    tls: TlsReferenceStatic,
    stack: Stack<'static>,
    space: &TlsConfigSpace,
    clock_is_set: bool,
    rx_buffer: &'buf mut [u8],
    tx_buffer: &'buf mut [u8],
    host: &'h core::ffi::CStr,
    port: u16,
) -> Result<Session<'h, TcpSocket<'buf>>, ClientTlsError> {
    if !clock_is_set { return Err(ClientTlsError::ClockUnsynced); }
    let ca = TlsService::new(EspCrypto).trusted_ca(space).await.ok_or(ClientTlsError::NoCa)?;
    crate::embassy::connect_client(tls, stack, rx_buffer, tx_buffer, host, port, &ca)
        .await
        .map_err(|e| match e {
            crate::embassy::ClientConnectError::BadCa => ClientTlsError::BadCa,
            crate::embassy::ClientConnectError::Dns => ClientTlsError::Dns,
            crate::embassy::ClientConnectError::Tcp(e) => ClientTlsError::Tcp(e),
            crate::embassy::ClientConnectError::Handshake(e) => ClientTlsError::Handshake(e),
        })
}
