#![no_std]
#![allow(async_fn_in_trait)]

//! Portable Wi-Fi component: durable credentials, reconnection and
//! reprovisioning. Radio, DHCP and network-stack types belong to an adapter.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Debug;
use iobewi_config_space::{Budget, ConfigBackend, ConfigSpace};
use log::{info, warn};

const CONFIG_MAGIC: &[u8; 4] = b"WFC1";
const CONFIG_HEADER_LEN: usize = 6;
/// Unchanged durable budget for station credentials.
pub const CONFIG_BUDGET: Budget = Budget::new(128);

pub struct Network {
    pub ssid: String,
    pub signal_strength: i8,
    pub secured: bool,
}

/// The platform supplies radio and network mechanics; the service controls
/// which credentials become authoritative after a connection attempt.
pub trait WifiTransport {
    type Address;
    /// Opaque handle to whatever network stack the platform runs once
    /// online (an `embassy_net::Stack`, a different runtime's socket
    /// manager, ...). This crate never interprets it -- it only carries it
    /// from the transport up to [`WifiManager`]'s own caller.
    type NetworkHandle: Copy;

    async fn connect(&mut self, ssid: &str, password: String) -> bool;
    async fn scan(&mut self) -> Vec<Network>;
    fn ip(&self) -> Option<Self::Address>;
    fn network_handle(&self) -> Option<Self::NetworkHandle>;
    fn is_online(&self) -> bool;
}

/// Consumer-facing capability: the functional operations a Wi-Fi
/// provisioning workflow (e.g. Improv Serial) needs. Deliberately narrower
/// than [`WifiTransport`] (the platform-facing port `WifiManager` itself
/// consumes) -- a provisioning UI has no business touching durable-config
/// internals, only scanning, provisioning, and reading the resulting state.
#[allow(async_fn_in_trait)]
pub trait WifiProvisioning {
    type Address: core::fmt::Display;
    type NetworkHandle: Copy;

    async fn scan(&mut self) -> Vec<Network>;
    async fn provision(&mut self, ssid: &str, password: String) -> bool;

    fn address(&self) -> Option<Self::Address>;
    fn network_handle(&self) -> Option<Self::NetworkHandle>;
    fn is_online(&self) -> bool;
}

#[derive(Clone)]
struct WifiConfig {
    ssid: String,
    password: String,
}

impl WifiConfig {
    fn encode(&self) -> Option<Vec<u8>> {
        let ssid_len = u8::try_from(self.ssid.len()).ok()?;
        let password_len = u8::try_from(self.password.len()).ok()?;
        let total = CONFIG_HEADER_LEN.checked_add(self.ssid.len())?.checked_add(self.password.len())?;
        if total > CONFIG_BUDGET.max_bytes() { return None; }
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(CONFIG_MAGIC);
        out.push(ssid_len);
        out.push(password_len);
        out.extend_from_slice(self.ssid.as_bytes());
        out.extend_from_slice(self.password.as_bytes());
        Some(out)
    }

    fn decode(raw: &[u8]) -> Option<Self> {
        if raw.len() < CONFIG_HEADER_LEN || &raw[..4] != CONFIG_MAGIC { return None; }
        let ssid_len = raw[4] as usize;
        let password_len = raw[5] as usize;
        let expected = CONFIG_HEADER_LEN.checked_add(ssid_len)?.checked_add(password_len)?;
        if raw.len() != expected { return None; }
        let ssid_end = CONFIG_HEADER_LEN + ssid_len;
        let ssid = core::str::from_utf8(&raw[CONFIG_HEADER_LEN..ssid_end]).ok()?;
        let password = core::str::from_utf8(&raw[ssid_end..]).ok()?;
        Some(Self { ssid: String::from(ssid), password: String::from(password) })
    }
}

pub async fn is_provisioned<B: ConfigBackend>(space: &ConfigSpace<B>) -> bool {
    match space.load().await {
        Ok(Some(snapshot)) => WifiConfig::decode(&snapshot.data)
            .is_some_and(|config| !config.ssid.is_empty()),
        _ => false,
    }
}

pub struct WifiManager<T, B: ConfigBackend> {
    transport: T,
    config: ConfigSpace<B>,
}

impl<T: WifiTransport, B: ConfigBackend> WifiManager<T, B>
where
    B::Error: Debug,
{
    pub fn new(transport: T, config: ConfigSpace<B>) -> Self {
        Self { transport, config }
    }

    async fn saved_config(&self) -> Option<WifiConfig> {
        match self.config.load().await {
            Ok(Some(snapshot)) => match WifiConfig::decode(&snapshot.data) {
                Some(config) => Some(config),
                None => {
                    warn!("Wi-Fi: stored config generation={} has an unsupported/corrupt schema", snapshot.generation);
                    None
                }
            },
            Ok(None) => None,
            Err(e) => { warn!("Wi-Fi: config-space load failed: {e:?}"); None }
        }
    }

    pub async fn reconnect_saved(&mut self) -> bool {
        let Some(config) = self.saved_config().await else { return false; };
        info!("Wi-Fi: reconnecting to saved SSID={}", config.ssid);
        self.transport.connect(&config.ssid, config.password).await
    }

    async fn restore_previous(&mut self, previous: Option<WifiConfig>) {
        let Some(previous) = previous else { return; };
        info!("Wi-Fi: restoring previous SSID={} after failed reprovision", previous.ssid);
        if !self.transport.connect(&previous.ssid, previous.password).await {
            warn!("Wi-Fi: previous network could not be restored");
        }
    }

    /// Publish credentials only after association, DHCP and atomic commit.
    pub async fn provision(&mut self, ssid: &str, password: String) -> bool {
        let previous = self.saved_config().await;
        if !self.transport.connect(ssid, password.clone()).await {
            self.restore_previous(previous).await;
            return false;
        }
        let candidate = WifiConfig { ssid: String::from(ssid), password };
        let Some(encoded) = candidate.encode() else {
            warn!("Wi-Fi: candidate credentials exceed config-space schema limits");
            self.restore_previous(previous).await;
            return false;
        };
        match self.config.commit(&encoded).await {
            Ok(generation) => { info!("Wi-Fi: configuration committed generation={generation}"); true }
            Err(e) => {
                warn!("Wi-Fi: connected, but durable config commit failed: {e:?}");
                self.restore_previous(previous).await;
                false
            }
        }
    }

    pub async fn scan(&mut self) -> Vec<Network> { self.transport.scan().await }
    pub fn ip(&self) -> Option<T::Address> { self.transport.ip() }
    pub fn network_handle(&self) -> Option<T::NetworkHandle> { self.transport.network_handle() }
    pub fn is_online(&self) -> bool { self.transport.is_online() }
}

/// Delegates straight to `WifiManager`'s own methods -- no policy lives
/// here, this only narrows the surface a provisioning workflow sees.
impl<T: WifiTransport, B: ConfigBackend> WifiProvisioning for WifiManager<T, B>
where
    B::Error: Debug,
    T::Address: core::fmt::Display,
{
    type Address = T::Address;
    type NetworkHandle = T::NetworkHandle;

    async fn scan(&mut self) -> Vec<Network> {
        self.transport.scan().await
    }

    async fn provision(&mut self, ssid: &str, password: String) -> bool {
        // Explicit associated-function syntax, not `self.provision(...)`:
        // this impl and the inherent one share the method name, and this
        // makes unambiguous which one carries the real
        // connect+commit+restore-on-failure policy.
        Self::provision(self, ssid, password).await
    }

    fn address(&self) -> Option<Self::Address> {
        self.transport.ip()
    }

    fn network_handle(&self) -> Option<Self::NetworkHandle> {
        self.transport.network_handle()
    }

    fn is_online(&self) -> bool {
        self.transport.is_online()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_wfc1_bytes_round_trip_without_schema_change() {
        let raw = b"WFC1\x03\x04labpass";
        let config = WifiConfig::decode(raw).unwrap();
        assert_eq!(config.ssid, "lab");
        assert_eq!(config.password, "pass");
        assert_eq!(config.encode().unwrap(), raw);
        assert!(WifiConfig::decode(b"WFC1\x03\x04labpas").is_none());
    }
}
