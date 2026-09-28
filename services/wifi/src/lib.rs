#![no_std]

//! Reusable ESP Wi-Fi station transport.
//!
//! Owns only Wi-Fi/network mechanics:
//!
//! - lazy radio initialization;
//! - station scans with strongest-SSID/BSSID selection;
//! - association;
//! - DHCP;
//! - Embassy network runner;
//! - reporting the resulting IP-capable stack.
//!
//! It deliberately does not own credential persistence, provisioning
//! protocols, TLS, HTTP, heartbeat/log services or application supervision.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources};
use esp_hal::peripherals::WIFI;
use esp_radio::wifi::{
    AuthenticationMethod, Config, Interface, WifiController, scan::ScanConfig, sta::StationConfig,
};
use log::{info, warn};

/// An access point found by [`WifiManager::scan`].
pub struct Network {
    pub ssid: String,
    pub signal_strength: i8,
    pub secured: bool,
}

struct Radio {
    controller: WifiController<'static>,
    stack: Stack<'static>,
}

/// Wi-Fi station transport.
///
/// The caller supplies the Embassy socket resources because socket-set sizing
/// is application policy. The manager consumes them only when the radio is
/// first initialized.
pub struct WifiManager<const SOCKETS: usize> {
    peripheral: Option<WIFI<'static>>,
    spawner: Spawner,
    resources: Option<&'static mut StackResources<SOCKETS>>,
    radio: Option<Radio>,
    strongest_bssid: Vec<(String, [u8; 6])>,
}

impl<const SOCKETS: usize> WifiManager<SOCKETS> {
    pub fn new(
        peripheral: WIFI<'static>,
        spawner: Spawner,
        resources: &'static mut StackResources<SOCKETS>,
    ) -> Self {
        Self {
            peripheral: Some(peripheral),
            spawner,
            resources: Some(resources),
            radio: None,
            strongest_bssid: Vec::new(),
        }
    }

    /// The device's current IPv4 address, if online.
    pub fn ip(&self) -> Option<embassy_net::Ipv4Address> {
        Some(self.radio.as_ref()?.stack.config_v4()?.address.address())
    }

    /// Returns the IP-capable network stack once DHCP has configured it.
    pub fn ip_stack(&self) -> Option<Stack<'static>> {
        let radio = self.radio.as_ref()?;
        radio.stack.config_v4()?;
        Some(radio.stack)
    }

    pub fn is_online(&self) -> bool {
        self.ip_stack().is_some()
    }

    fn radio(&mut self) -> Option<&mut Radio> {
        if self.radio.is_none() {
            let (mut controller, interfaces) =
                match esp_radio::wifi::new(self.peripheral.take()?, Default::default()) {
                    Ok(parts) => parts,
                    Err(e) => {
                        warn!("Wi-Fi init failed: {e:?}");
                        return None;
                    }
                };

            if let Err(e) = controller.set_config(&Config::Station(StationConfig::default())) {
                warn!("Wi-Fi start failed: {e:?}");
                return None;
            }

            let resources = self.resources.take()?;
            let seed = esp_hal::time::Instant::now().duration_since_epoch().as_micros();
            let (stack, runner) = embassy_net::new(
                interfaces.station,
                embassy_net::Config::dhcpv4(Default::default()),
                resources,
                seed,
            );
            self.spawner.spawn(net_task(runner).unwrap());
            self.radio = Some(Radio { controller, stack });
        }

        self.radio.as_mut()
    }

    /// Scans for networks, one entry per SSID, keeping the strongest BSSID.
    pub async fn scan(&mut self) -> Vec<Network> {
        let Some(radio) = self.radio() else {
            return Vec::new();
        };

        let access_points = match radio
            .controller
            .scan_async(&ScanConfig::default().with_max(20))
            .await
        {
            Ok(access_points) => access_points,
            Err(e) => {
                warn!("Wi-Fi scan failed: {e:?}");
                return Vec::new();
            }
        };

        let mut strongest: Vec<(String, [u8; 6], i8, bool)> = Vec::new();
        for ap in &access_points {
            let ssid = ap.ssid.as_str();
            if ssid.is_empty() {
                continue;
            }

            let secured = !matches!(ap.auth_method, None | Some(AuthenticationMethod::None));
            match strongest.iter_mut().find(|(known_ssid, ..)| known_ssid == ssid) {
                Some((_, _, signal_strength, _)) if *signal_strength >= ap.signal_strength => {}
                Some(entry) => *entry = (String::from(ssid), ap.bssid, ap.signal_strength, secured),
                None => strongest.push((String::from(ssid), ap.bssid, ap.signal_strength, secured)),
            }
        }

        self.strongest_bssid = strongest
            .iter()
            .map(|(ssid, bssid, ..)| (ssid.clone(), *bssid))
            .collect();

        strongest
            .into_iter()
            .map(|(ssid, _, signal_strength, secured)| Network {
                ssid,
                signal_strength,
                secured,
            })
            .collect()
    }

    /// Connects and waits for DHCP. Pins to the strongest BSSID seen for this
    /// SSID in the last scan, if available.
    pub async fn connect(&mut self, ssid: &str, password: String) -> bool {
        let bssid = self
            .strongest_bssid
            .iter()
            .find(|(known_ssid, _)| known_ssid == ssid)
            .map(|(_, bssid)| *bssid);

        if bssid.is_none() {
            warn!("Wi-Fi: no scan result for {ssid}, letting the radio pick an access point");
        }

        let Some(radio) = self.radio() else {
            return false;
        };

        // Reprovisioning may happen while the station is already associated
        // (for example through Improv Serial). esp-radio does not treat
        // set_config()+connect_async() as a roam/reconfigure operation on an
        // already-connected station: explicitly tear the old association down
        // first, and wait until embassy-net has dropped the old DHCP config so
        // wait_config_up() below cannot return immediately with a stale lease.
        if radio.controller.is_connected() {
            info!("Wi-Fi: disconnecting current association before reconfiguration");
            if let Err(e) = radio.controller.disconnect_async().await {
                warn!("Wi-Fi: disconnect before reconfiguration failed: {e:?}");
                return false;
            }
        }
        if radio.stack.is_config_up() {
            radio.stack.wait_config_down().await;
        }

        let mut config = StationConfig::default()
            .with_ssid(ssid)
            .with_password(password);
        if let Some(bssid) = bssid {
            config = config.with_bssid(bssid);
        }

        if radio
            .controller
            .set_config(&Config::Station(config))
            .is_err()
            || radio.controller.connect_async().await.is_err()
        {
            warn!("Wi-Fi: connection to {ssid} failed");
            return false;
        }

        radio.stack.wait_config_up().await;
        info!("Wi-Fi connected, ip = {:?}", radio.stack.config_v4());
        true
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface<'static>>) -> ! {
    runner.run().await
}
