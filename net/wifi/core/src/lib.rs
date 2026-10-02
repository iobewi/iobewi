#![no_std]
#![allow(async_fn_in_trait)]

//! Portable Wi-Fi contracts. No persistence, provisioning policy, radio,
//! DHCP or network-stack types: a platform driver implements
//! [`WifiTransport`]; a manager (see `iobewi-wifi-manager`) consumes it and
//! exposes [`WifiProvisioning`] to provisioning workflows.
//!
//! "Online" means what the transport reports through [`WifiTransport::is_online`]:
//! for the ESP driver, associated *and* IPv4 configured by DHCP.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

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
    /// Resolves when the link/IP configuration is lost (immediately if it is
    /// already down). A pure event primitive: it neither retries nor
    /// reconnects -- that policy belongs to the manager.
    async fn wait_down(&mut self);
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
