#![no_std]
#![allow(async_fn_in_trait)]

//! Portable Wi-Fi component: durable credentials, reconnection and
//! reprovisioning. Radio, DHCP and network-stack types belong to an adapter.

extern crate alloc;
#[cfg(test)]
extern crate std;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Debug;
use iobewi_config_space::{Budget, ConfigBackend, ConfigSpace};
use iobewi_wifi_core::{Network, WifiProvisioning, WifiTransport};
use log::{info, warn};

const CONFIG_MAGIC: &[u8; 4] = b"WFC1";
const CONFIG_HEADER_LEN: usize = 6;
/// Unchanged durable budget for station credentials.
pub const CONFIG_BUDGET: Budget = Budget::new(128);

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
    use alloc::collections::{BTreeMap, VecDeque};
    use alloc::string::ToString;
    use alloc::rc::Rc;
    use core::cell::RefCell;
    use core::future::Future;
    use core::task::{Context, Poll, Waker};
    use iobewi_config_space::{ConfigManager, Snapshot};

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = core::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    #[derive(Default)]
    struct Mem {
        values: BTreeMap<String, Snapshot>,
        generation: u64,
        fail_commit: bool,
    }

    #[derive(Clone, Default)]
    struct MemBackend(Rc<RefCell<Mem>>);

    impl ConfigBackend for MemBackend {
        type Error = ();
        fn capacity_units(&self) -> usize { 4096 }
        fn reservation_units(&self, _: &str, b: Budget) -> Option<usize> { Some(b.max_bytes()) }
        async fn load(&self, space: &str) -> Result<Option<Snapshot>, ()> {
            Ok(self.0.borrow().values.get(space).cloned())
        }
        async fn commit(&self, space: &str, data: &[u8]) -> Result<u64, ()> {
            let mut m = self.0.borrow_mut();
            if m.fail_commit { return Err(()); }
            m.generation += 1;
            let generation = m.generation;
            m.values.insert(space.to_string(), Snapshot { generation, data: data.to_vec() });
            Ok(generation)
        }
        async fn clear(&self, space: &str) -> Result<u64, ()> {
            let mut m = self.0.borrow_mut();
            m.generation += 1;
            m.values.remove(space);
            Ok(m.generation)
        }
    }

    /// Scripted transport: each `connect` pops the next outcome (default: fail)
    /// and records `(ssid, password)`.
    #[derive(Default)]
    struct FakeState {
        outcomes: VecDeque<bool>,
        connects: std::vec::Vec<(String, String)>,
        online: bool,
    }

    #[derive(Clone, Default)]
    struct Fake(Rc<RefCell<FakeState>>);

    impl WifiTransport for Fake {
        type Address = u32;
        type NetworkHandle = u8;
        async fn connect(&mut self, ssid: &str, password: String) -> bool {
            let mut s = self.0.borrow_mut();
            s.connects.push((ssid.to_string(), password));
            let ok = s.outcomes.pop_front().unwrap_or(false);
            s.online = ok;
            ok
        }
        async fn scan(&mut self) -> Vec<Network> {
            alloc::vec![Network { ssid: "lab".to_string(), signal_strength: -40, secured: true }]
        }
        fn ip(&self) -> Option<u32> { self.0.borrow().online.then_some(7) }
        fn network_handle(&self) -> Option<u8> { self.0.borrow().online.then_some(1) }
        fn is_online(&self) -> bool { self.0.borrow().online }
    }

    fn setup(outcomes: &[bool]) -> (WifiManager<Fake, MemBackend>, Fake, MemBackend) {
        let backend = MemBackend::default();
        let fake = Fake::default();
        fake.0.borrow_mut().outcomes = outcomes.iter().copied().collect();
        let space = ConfigManager::new(backend.clone()).claim("wifi", CONFIG_BUDGET).unwrap();
        (WifiManager::new(fake.clone(), space), fake, backend)
    }

    fn stored(b: &MemBackend) -> Option<std::vec::Vec<u8>> {
        b.0.borrow().values.get("wifi").map(|s| s.data.clone())
    }

    fn seed(b: &MemBackend, ssid: &str, pw: &str) {
        let raw = WifiConfig { ssid: ssid.to_string(), password: pw.to_string() }.encode().unwrap();
        b.0.borrow_mut().values.insert("wifi".to_string(), Snapshot { generation: 1, data: raw });
    }

    #[test]
    fn legacy_wfc1_bytes_round_trip_without_schema_change() {
        let raw = b"WFC1\x03\x04labpass";
        let config = WifiConfig::decode(raw).unwrap();
        assert_eq!(config.ssid, "lab");
        assert_eq!(config.password, "pass");
        assert_eq!(config.encode().unwrap(), raw);
        assert!(WifiConfig::decode(b"WFC1\x03\x04labpas").is_none());
    }

    #[test]
    fn credentials_absent_means_unprovisioned_and_no_connect_attempt() {
        let (mut m, fake, _b) = setup(&[true]);
        assert!(!block_on(m.reconnect_saved()));
        assert!(fake.0.borrow().connects.is_empty());
        assert!(!m.is_online());
    }

    #[test]
    fn corrupt_or_empty_ssid_credentials_are_not_provisioned() {
        let (mut m, fake, b) = setup(&[true]);
        b.0.borrow_mut().values.insert("wifi".into(), Snapshot { generation: 1, data: b"junk".to_vec() });
        assert!(!block_on(m.reconnect_saved()));
        assert!(fake.0.borrow().connects.is_empty());
        seed(&b, "", "x");
        let space = ConfigManager::new(b.clone()).claim("wifi", CONFIG_BUDGET).unwrap();
        assert!(!block_on(is_provisioned(&space)));
    }

    #[test]
    fn saved_credentials_reconnect_with_the_stored_values() {
        let (mut m, fake, b) = setup(&[true]);
        seed(&b, "lab", "secret");
        assert!(block_on(m.reconnect_saved()));
        assert_eq!(fake.0.borrow().connects, [("lab".to_string(), "secret".to_string())]);
        assert!(m.is_online());
        assert_eq!(m.ip(), Some(7));
        assert_eq!(m.network_handle(), Some(1));
        let space = ConfigManager::new(b).claim("wifi", CONFIG_BUDGET).unwrap();
        assert!(block_on(is_provisioned(&space)));
    }

    #[test]
    fn failed_reconnect_is_reported_and_retry_can_succeed() {
        let (mut m, fake, b) = setup(&[false, true]);
        seed(&b, "lab", "pw");
        assert!(!block_on(m.reconnect_saved()));
        assert!(!m.is_online());
        assert!(block_on(m.reconnect_saved()));
        assert_eq!(fake.0.borrow().connects.len(), 2);
    }

    #[test]
    fn provision_commits_only_after_a_successful_connection() {
        let (mut m, _fake, b) = setup(&[true]);
        assert!(block_on(m.provision("home", "pw".to_string())));
        assert_eq!(stored(&b).unwrap(), b"WFC1\x04\x02homepw");
    }

    #[test]
    fn reprovision_failure_restores_the_previous_network_and_keeps_old_credentials() {
        // connect(new) fails, then restore(previous) succeeds.
        let (mut m, fake, b) = setup(&[false, true]);
        seed(&b, "old", "oldpw");
        assert!(!block_on(m.provision("new", "newpw".to_string())));
        let calls = fake.0.borrow().connects.clone();
        assert_eq!(calls, [("new".to_string(), "newpw".to_string()), ("old".to_string(), "oldpw".to_string())]);
        assert_eq!(stored(&b).unwrap(), b"WFC1\x03\x05oldoldpw");
        assert!(m.is_online());
    }

    #[test]
    fn reprovision_success_replaces_the_credentials_in_order() {
        let (mut m, fake, b) = setup(&[true]);
        seed(&b, "old", "oldpw");
        assert!(block_on(m.provision("new", "newpw".to_string())));
        // One connect only: the transport itself owns disconnect-before-reconfigure.
        assert_eq!(fake.0.borrow().connects, [("new".to_string(), "newpw".to_string())]);
        assert_eq!(stored(&b).unwrap(), b"WFC1\x03\x05newnewpw");
    }

    #[test]
    fn commit_failure_after_connect_restores_previous_and_reports_failure() {
        let (mut m, fake, b) = setup(&[true, true]);
        seed(&b, "old", "oldpw");
        b.0.borrow_mut().fail_commit = true;
        assert!(!block_on(m.provision("new", "newpw".to_string())));
        assert_eq!(fake.0.borrow().connects.len(), 2);
        assert_eq!(fake.0.borrow().connects[1].0, "old");
        assert_eq!(stored(&b).unwrap(), b"WFC1\x03\x05oldoldpw");
    }

    #[test]
    fn oversized_credentials_are_rejected_without_a_commit() {
        let (mut m, _fake, b) = setup(&[true, true]);
        let long = "p".repeat(200);
        assert!(!block_on(m.provision("home", long)));
        assert!(stored(&b).is_none());
    }

    #[test]
    fn provisioning_capability_delegates_to_the_manager_policy() {
        let (mut m, _fake, b) = setup(&[true]);
        assert!(block_on(WifiProvisioning::provision(&mut m, "home", "pw".to_string())));
        assert!(stored(&b).is_some());
        assert_eq!(WifiProvisioning::address(&m), Some(7));
        assert_eq!(block_on(WifiProvisioning::scan(&mut m)).len(), 1);
    }
}
