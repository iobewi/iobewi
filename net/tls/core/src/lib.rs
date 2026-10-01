#![no_std]

//! TLS network contracts, free of identity/ConfigSpace policy.
//!
//! Currently the guarantee carried by a secure outbound connector.

use iobewi_net_io::Connector;

/// A [`Connector`] that promises every connection it returns is
/// authenticated and encrypted, with a fail-closed trust policy.
///
/// A caller asks for a connection to `host:port` and gets back such a stream
/// without ever knowing how DNS, TCP, the TLS handshake, certificates, the
/// clock, or the underlying network stack are implemented. Those are the
/// implementation's own construction-time concern -- trust material (CA,
/// clock) is injected when the implementation is built, never passed to
/// `connect()` itself, so the same fail-closed policy applies to every call
/// without the caller having to know it exists.
///
/// This is a marker: it adds no methods. Implementors opt in explicitly, so a
/// plaintext connector can never satisfy a bound on `SecureClientTransport`
/// by accident.
///
/// This crate deliberately does not cover:
/// - the TLS *server* side (accepting connections) -- see `iobewi-https`'s
///   `TlsListener`;
/// - identity/CA/crypto policy -- see `iobewi-tls`'s `TlsCrypto`;
/// - entropy -- not a property of a secure transport; a consumer that needs
///   it (e.g. WebSocket frame masking) asks for it as its own capability.
pub trait SecureClientTransport: Connector {}
