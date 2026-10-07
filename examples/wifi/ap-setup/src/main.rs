#![no_std]
#![no_main]

//! Example: provisioning through the soft access point (ADR-0017): an unconfigured device opens the
//! access point, serves a form, calls the REAL `WifiManager::provision` with the access point
//! still up, then stops the access point and lets `maintain` bring the station up from the
//! saved credentials. Everything under test is the production code of `iobewi-wifi-*` and
//! `iobewi-esp-wifi`; only the page and this sequence are product-side.
//!
//! Credentials live in RAM (nothing is written to flash): a power cycle starts unconfigured again.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_net::{Stack, StackResources};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use esp_println::println;
use iobewi_config_space::{Budget, ConfigBackend, ConfigManager, Snapshot};
use iobewi_esp_tcp::EspTcpListener;
use iobewi_esp_wifi::WifiManager as EspWifi;
use iobewi_http_server::{HttpRouter, serve_forever_io};
use iobewi_wifi_core::{AccessPointConfig, Network};
use iobewi_wifi_manager::{CONFIG_BUDGET, LinkObserver, Sleep, WifiManager};
use picoserve::extract::Form;
use picoserve::response::Response;
use picoserve::routing::{get, post};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();

/// Station sockets: kept as in a product; the access point needs the DHCP server (1 UDP) and the
/// page's listener (1 TCP) plus margin.
const STA_SOCKETS: usize = 4;
const AP_SOCKETS: usize = 4;
static STA: StaticCell<StackResources<STA_SOCKETS>> = StaticCell::new();
static AP: StaticCell<StackResources<AP_SOCKETS>> = StaticCell::new();

const SETUP_SSID: &str = "IOBEWI-Setup";
/// Test-only passphrase, overridable at build time (`SETUP_AP_PASSWORD`). A product must use a
/// unique secret (ADR-0017).
const SETUP_PASSWORD: &str = match option_env!("SETUP_AP_PASSWORD") {
    Some(value) => value,
    None => "123456789",
};

const PAGE: &str = r#"<!doctype html><html lang="fr"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>IOBEWI Setup</title>
<style>body{font-family:system-ui,sans-serif;max-width:28rem;margin:2rem auto;padding:0 1rem}
input,button,select{font:inherit;padding:.5rem;width:100%;box-sizing:border-box}
select{font-family:ui-monospace,monospace;font-size:.9rem}</style></head><body>
<h1>Configuration Wi-Fi</h1>
<p><label>Réseaux détectés (du plus fort au plus faible)<br>
<select id="nets" size="7"></select></label></p>
<p><button id="rescan" type="button">Actualiser la liste</button></p>
<form id="f"><p><label>Réseau (SSID)<br><input name="ssid" required maxlength="32" autocomplete="off"></label></p>
<p><label>Mot de passe<br><input name="password" type="password" maxlength="63" autocomplete="off"></label></p>
<p><button>Connecter</button></p></form><p id="s"></p>
<script>
const s=document.getElementById('s'),f=document.getElementById('f'),
 list=document.getElementById('nets'),btn=document.getElementById('rescan');
const wait=ms=>new Promise(r=>setTimeout(r,ms));
// Network names are rendered with textContent only: neighbours' SSIDs are untrusted input.
async function load(){const lines=(await (await fetch('/networks')).text()).split('\n'),state=lines.shift();
 list.textContent='';
 for(const l of lines){if(!l)continue;const [rssi,sec,enc]=l.split('\t'),ssid=decodeURIComponent(enc),
  q=Math.max(0,Math.min(100,2*(+rssi+100))),o=document.createElement('option');
  o.value=ssid;o.textContent=(sec==='1'?'\u{1F512} ':'\u00A0\u00A0\u00A0 ')+ssid+'  '+rssi+' dBm ('+q+'%)';list.appendChild(o);}
 return state;}
list.onchange=()=>{f.ssid.value=list.value;f.password.focus();};
async function rescan(){btn.disabled=true;s.textContent='Recherche… (la connexion peut se couper un instant)';
 try{await fetch('/scan');for(let i=0;i<20;i++){await wait(1000);if(await load()==='ready')break;}}catch(x){}
 btn.disabled=false;s.textContent='';}
btn.onclick=rescan;load().catch(()=>{});
f.onsubmit=async e=>{e.preventDefault();s.textContent='Envoi…';
 try{const r=await fetch('/connect',{method:'POST',body:new URLSearchParams(new FormData(f))});
  s.textContent=await r.text();poll();}catch(x){s.textContent='Erreur: '+x;}};
async function poll(){for(;;){await wait(1500);
 try{const t=await (await fetch('/status')).text();s.textContent=t;
  if(!t.startsWith('connecting'))break;}
 catch(x){s.textContent='Point d\u2019accès fermé : normal après un succès.';break;}}}
</script></body></html>"#;

#[derive(serde::Deserialize)]
struct Credentials {
    ssid: String,
    password: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Idle,
    Connecting,
    Connected([u8; 4]),
    Failed,
}

static STATUS: Mutex<CriticalSectionRawMutex, Cell<Status>> = Mutex::new(Cell::new(Status::Idle));
static REQUEST: Signal<CriticalSectionRawMutex, (String, String)> = Signal::new();
static STOP_PAGE: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static SCAN_REQUEST: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// Last scan, strongest signal first. Written by the main sequence (it owns the radio).
static NETWORKS: Mutex<CriticalSectionRawMutex, RefCell<Vec<Network>>> =
    Mutex::new(RefCell::new(Vec::new()));
static SCANNING: Mutex<CriticalSectionRawMutex, Cell<bool>> = Mutex::new(Cell::new(false));

/// `rssi<TAB>secured<TAB>percent-encoded ssid` per line, after a `scanning`/`ready` line.
/// Percent-encoding keeps any byte of a (neighbour's) SSID out of the line structure.
fn networks_text() -> String {
    let mut text = String::from(if SCANNING.lock(|c| c.get()) {
        "scanning"
    } else {
        "ready"
    });
    NETWORKS.lock(|list| {
        for network in list.borrow().iter() {
            text.push('\n');
            text.push_str(&alloc::format!(
                "{}\t{}\t",
                network.signal_strength,
                if network.secured { 1 } else { 0 }
            ));
            for byte in network.ssid.bytes() {
                if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                    text.push(byte as char);
                } else {
                    text.push_str(&alloc::format!("%{byte:02X}"));
                }
            }
        }
    });
    text
}

/// Scans (the strongest entry per SSID, as the driver returns it) and stores the result sorted
/// by signal strength, strongest first. Only counts are logged, never network names.
async fn refresh_networks<T: iobewi_wifi_core::WifiTransport, B: ConfigBackend>(
    wifi: &mut WifiManager<T, B>,
) where
    B::Error: core::fmt::Debug,
{
    SCANNING.lock(|c| c.set(true));
    let mut found = wifi.scan().await;
    found.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));
    println!("SETUP: t={}ms scan: {} networks", now_ms(), found.len());
    NETWORKS.lock(|list| *list.borrow_mut() = found);
    SCANNING.lock(|c| c.set(false));
}

fn set_status(status: Status) {
    STATUS.lock(|cell| cell.set(status));
}

fn status() -> Status {
    STATUS.lock(|cell| cell.get())
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // No message: a panic payload could carry configuration text.
    println!("SETUP: panic");
    loop {
        core::hint::spin_loop();
    }
}

/// The page, served on the access point's own network until told to stop.
#[embassy_executor::task]
async fn setup_page(stack: Stack<'static>) {
    let router = HttpRouter::new()
        .route(
            "/",
            get(|| async { Response::ok(PAGE).with_content_type("text/html; charset=utf-8") }),
        )
        .route(
            "/networks",
            get(|| async {
                Response::ok(networks_text()).with_content_type("text/plain; charset=utf-8")
            }),
        )
        .route(
            "/scan",
            get(|| async {
                SCANNING.lock(|c| c.set(true));
                SCAN_REQUEST.signal(());
                Response::ok("ok").with_content_type("text/plain; charset=utf-8")
            }),
        )
        .route(
            "/status",
            get(|| async {
                let text = match status() {
                    Status::Idle => String::from("en attente"),
                    Status::Connecting => String::from("connecting : tentative de connexion…"),
                    Status::Connected([a, b, c, d]) => {
                        alloc::format!("connecté, adresse {a}.{b}.{c}.{d}")
                    }
                    Status::Failed => String::from("échec : vérifie le réseau et le mot de passe"),
                };
                Response::ok(text).with_content_type("text/plain; charset=utf-8")
            }),
        )
        .route(
            "/connect",
            post(|Form(credentials): Form<Credentials>| async move {
                let text = if status() == Status::Connecting || credentials.ssid.is_empty() {
                    "occupé ou SSID vide"
                } else {
                    set_status(Status::Connecting);
                    REQUEST.signal((credentials.ssid, credentials.password));
                    "connecting : demande reçue"
                };
                Response::ok(text).with_content_type("text/plain; charset=utf-8")
            }),
        );
    let mut rx = [0u8; 1024];
    let mut tx = [0u8; 1024];
    let mut listener = EspTcpListener::new(stack, 80, &mut rx, &mut tx);
    // Stopped by the sequence before the access point goes away (the product revokes its own service).
    select(serve_forever_io(&mut listener, &router), STOP_PAGE.wait()).await;
    println!("SETUP: page stopped");
}

/// RAM-only config backend: the proof must not write flash.
#[derive(Clone, Default)]
struct RamBackend(Rc<RefCell<(BTreeMap<String, Snapshot>, u64)>>);

impl ConfigBackend for RamBackend {
    type Error = ();
    fn capacity_units(&self) -> usize {
        4096
    }
    fn reservation_units(&self, _: &str, budget: Budget) -> Option<usize> {
        Some(budget.max_bytes())
    }
    async fn load(&self, space: &str) -> Result<Option<Snapshot>, ()> {
        Ok(self.0.borrow().0.get(space).cloned())
    }
    async fn commit(&self, space: &str, data: &[u8]) -> Result<u64, ()> {
        let mut state = self.0.borrow_mut();
        state.1 += 1;
        let generation = state.1;
        state.0.insert(
            space.to_string(),
            Snapshot {
                generation,
                data: data.to_vec(),
            },
        );
        Ok(generation)
    }
    async fn clear(&self, space: &str) -> Result<u64, ()> {
        let mut state = self.0.borrow_mut();
        state.1 += 1;
        state.0.remove(space);
        Ok(state.1)
    }
}

struct EmbassySleep;
impl Sleep for EmbassySleep {
    async fn sleep_ms(&self, ms: u32) {
        Timer::after(Duration::from_millis(ms as u64)).await;
    }
}

struct Observer;
impl LinkObserver<Stack<'static>> for Observer {
    fn link_down(&mut self) {
        println!("SETUP: station link down, reconnecting");
    }
    fn ready(&mut self, network: Stack<'static>) {
        match network.config_v4() {
            Some(config) => println!("SETUP: station READY ip={}", config.address.address()),
            None => println!("SETUP: station ready (no IPv4 config?)"),
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: 160 * 1024);
    let timer = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timer.timer0, peripherals.FROM_CPU_INTR0);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    // The code under test: the ESP transport with an access point, behind the portable manager.
    let transport = EspWifi::new(peripherals.WIFI, spawner, STA.init(StackResources::new()))
        .with_access_point(AP.init(StackResources::new()));
    let space = ConfigManager::new(RamBackend::default())
        .claim("wifi", CONFIG_BUDGET)
        .unwrap();
    let mut wifi = WifiManager::new(transport, space);

    println!(
        "SETUP: t={}ms start (unconfigured, credentials in RAM only)",
        now_ms()
    );
    // First scan before the access point exists: nobody is connected yet, so nothing is disturbed.
    refresh_networks(&mut wifi).await;
    let access_point = AccessPointConfig::new(SETUP_SSID, SETUP_PASSWORD, 6).unwrap();
    if !wifi.start_access_point(&access_point).await {
        println!("SETUP: FAIL access point did not start");
        core::future::pending::<()>().await;
    }
    let Some(handle) = wifi.access_point_handle() else {
        println!("SETUP: FAIL no access point handle");
        core::future::pending::<()>().await;
        return;
    };
    println!(
        "SETUP: t={}ms access point ACTIVE: join \"{}\", open http://172.23.241.1/",
        now_ms(),
        SETUP_SSID
    );
    spawner.spawn(setup_page(handle).unwrap());

    loop {
        let (ssid, password) = match select(REQUEST.wait(), SCAN_REQUEST.wait()).await {
            Either::First(credentials) => credentials,
            Either::Second(()) => {
                refresh_networks(&mut wifi).await;
                continue;
            }
        };
        println!(
            "SETUP: t={}ms provisioning requested (ssid {} bytes), access point stays up",
            now_ms(),
            ssid.len()
        );
        if wifi.provision(&ssid, password).await {
            let address = wifi.ip().map(|a| a.octets()).unwrap_or([0; 4]);
            set_status(Status::Connected(address));
            println!(
                "SETUP: t={}ms provisioned, station connected, ip={}.{}.{}.{}",
                now_ms(),
                address[0],
                address[1],
                address[2],
                address[3]
            );
            // Let the page's status poll read the result before its network disappears.
            Timer::after_secs(8).await;
            STOP_PAGE.signal(());
            Timer::after_millis(300).await;
            println!(
                "SETUP: t={}ms stopping access point (the radio restarts)",
                now_ms()
            );
            wifi.stop_access_point().await;
            println!(
                "SETUP: t={}ms access point stopped, handing over to maintain",
                now_ms()
            );
            break;
        }
        set_status(Status::Failed);
        println!(
            "SETUP: t={}ms connection failed; access point stays up for a retry",
            now_ms()
        );
    }

    // From here the saved credentials drive the station, exactly as at a normal boot.
    let result = wifi.maintain(&EmbassySleep, &mut Observer).await;
    println!(
        "SETUP: maintain ended ({:?})",
        result == iobewi_wifi_manager::MaintainError::NotProvisioned
    );
    core::future::pending::<()>().await;
}

fn now_ms() -> u64 {
    embassy_time::Instant::now().as_millis()
}
