#![no_std]

//! Hardware-agnostic product logic for provisioning a device through its own soft access point
//! (ADR-0017). It consumes ports only: a transport that is both a [`WifiTransport`] and a
//! [`WifiAccessPoint`], a [`ConfigBackend`] and a [`ConnectionListener`] bound to the access
//! point's network. Nothing here names a chip, a HAL or a network stack; a target composition
//! package (see `esp32s3/`) owns the hardware and calls [`run`].
//!
//! Sequence: a boot with saved credentials joins the network directly and never opens the
//! access point. Without credentials: scan, open the access point, serve a form listing the
//! scanned networks (strongest first), call the manager's `provision` with the access point
//! still up, keep the answer readable for a few seconds, close the page, stop the access
//! point, then let `maintain` bring the station up from the saved credentials.

extern crate alloc;
#[cfg(test)]
extern crate std;

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt::{Debug, Display};

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use iobewi_config_space::{ConfigBackend, ConfigSpace};
use iobewi_http_server::{HttpRouter, serve_forever_io};
use iobewi_net_io::ConnectionListener;
use iobewi_wifi_core::{AccessPointConfig, Network, WifiAccessPoint, WifiTransport};
use iobewi_wifi_manager::{LinkObserver, Sleep, WifiManager, is_provisioned};
use log::{info, warn};
use picoserve::extract::Form;
use picoserve::response::Response;
use picoserve::routing::{get, post};

/// Gives the page a listener on the access point's network. A target implements it over its own
/// network stack. It lends from `self` so the buffers it owns can be reused if the setup runs
/// again.
pub trait PageListener<Handle> {
    type Listener<'a>: ConnectionListener
    where
        Self: 'a;

    fn listener(&mut self, access_point_network: Handle) -> Self::Listener<'_>;
}

const PAGE: &str = r#"<!doctype html><html lang="fr"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>IOBEWI Setup</title>
<style>body{font-family:system-ui,sans-serif;max-width:28rem;margin:2rem auto;padding:0 1rem}
input,button,select{font:inherit;padding:.5rem;width:100%;box-sizing:border-box}
select{font-family:ui-monospace,monospace;font-size:.9rem}</style></head><body>
<h1>Configuration Wi-Fi</h1>
<p><label>Réseaux détectés (du plus fort au plus faible)<br>
<select id="nets"><option value="">Recherche en cours…</option></select></label></p>
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
 list.textContent='';let n=0;
 const head=document.createElement('option');head.value='';list.appendChild(head);
 for(const l of lines){if(!l)continue;const [rssi,sec,enc]=l.split('\t'),ssid=decodeURIComponent(enc),
  q=Math.max(0,Math.min(100,2*(+rssi+100))),o=document.createElement('option');
  o.value=ssid;o.textContent=(sec==='1'?'\u{1F512} ':'\u00A0\u00A0\u00A0 ')+ssid+'  '+rssi+' dBm ('+q+'%)';list.appendChild(o);n++;}
 head.textContent=state==='scanning'&&!n?'Recherche en cours…':n?'— choisir un réseau ('+n+' détectés) —':'Aucun réseau trouvé : saisis le SSID';
 return state;}
list.onchange=()=>{if(list.value){f.ssid.value=list.value;f.password.focus();}};
async function rescan(){btn.disabled=true;s.textContent='Recherche… (la connexion peut se couper un instant)';
 try{await fetch('/scan');for(let i=0;i<20;i++){await wait(1000);if(await load()==='ready')break;}}catch(x){}
 btn.disabled=false;s.textContent='';}
btn.onclick=rescan;load().catch(x=>{s.textContent='Liste indisponible : '+x;});
// Joining the network moves the radio to the router's channel, and the access point follows:
// the link to this page can drop for a moment, so a failed request is not a failed connection.
f.onsubmit=async e=>{e.preventDefault();s.textContent='Envoi…';
 try{const r=await fetch('/connect',{method:'POST',body:new URLSearchParams(new FormData(f))});
  s.textContent=await r.text();}
 catch(x){s.textContent='Connexion en cours : la liaison peut se couper un instant (la carte change de canal)…';}
 poll();};
async function poll(){let fails=0;
 for(let i=0;i<80;i++){await wait(1500);
  try{const t=await (await fetch('/status')).text();fails=0;s.textContent=t;
   if(t.startsWith('en attente')&&i>3){s.textContent='La demande ne semble pas être arrivée : réessaie.';return;}
   if(!t.startsWith('connecting')&&!t.startsWith('en attente'))return;}
  catch(x){fails++;s.textContent='Connexion en cours… liaison coupée un instant ('+fails+')';
   if(fails>=8){s.textContent='Point d\u2019accès fermé ou injoignable. Si la carte s\u2019est connectée, c\u2019est normal : vérifie l\u2019UART ou ta box.';return;}}}}
</script></body></html>"#;

#[derive(serde::Deserialize)]
struct Credentials {
    ssid: String,
    password: String,
}

#[derive(Clone, PartialEq, Eq)]
enum Status {
    Idle,
    Connecting,
    Connected(String),
    Failed,
}

static STATUS: Mutex<CriticalSectionRawMutex, RefCell<Status>> =
    Mutex::new(RefCell::new(Status::Idle));
static REQUEST: Signal<CriticalSectionRawMutex, (String, String)> = Signal::new();
static SCAN_REQUEST: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// Last scan, strongest signal first. Written by the sequence (it owns the radio).
static NETWORKS: Mutex<CriticalSectionRawMutex, RefCell<Vec<Network>>> =
    Mutex::new(RefCell::new(Vec::new()));
static SCANNING: Mutex<CriticalSectionRawMutex, RefCell<bool>> = Mutex::new(RefCell::new(false));

fn set_status(status: Status) {
    STATUS.lock(|cell| *cell.borrow_mut() = status);
}

fn status() -> Status {
    STATUS.lock(|cell| cell.borrow().clone())
}

fn now_ms() -> u64 {
    Instant::now().as_millis()
}

/// Strongest signal first, so the form's list reads best-first.
fn rank(mut found: Vec<Network>) -> Vec<Network> {
    found.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));
    found
}

/// `scanning`/`ready`, then `rssi<TAB>secured<TAB>percent-encoded ssid` per line.
/// Percent-encoding keeps any byte of a (neighbour's) SSID out of the line structure.
fn networks_text(list: &[Network], scanning: bool) -> String {
    let mut text = String::from(if scanning { "scanning" } else { "ready" });
    for network in list {
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
    text
}

fn status_text(status: &Status) -> String {
    match status {
        Status::Idle => String::from("en attente"),
        Status::Connecting => String::from("connecting : tentative de connexion…"),
        Status::Connected(address) => alloc::format!("connecté, adresse {address}"),
        Status::Failed => String::from("échec : vérifie le réseau et le mot de passe"),
    }
}

/// Scans (the driver keeps the strongest entry per SSID) and stores the result best-first. Only
/// counts are logged, never network names.
async fn refresh_networks<T: WifiTransport, B: ConfigBackend>(wifi: &mut WifiManager<T, B>)
where
    B::Error: Debug,
{
    SCANNING.lock(|c| *c.borrow_mut() = true);
    let found = rank(wifi.scan().await);
    info!("SETUP: t={}ms scan: {} networks", now_ms(), found.len());
    NETWORKS.lock(|list| *list.borrow_mut() = found);
    SCANNING.lock(|c| *c.borrow_mut() = false);
}

struct EmbassySleep;
impl Sleep for EmbassySleep {
    async fn sleep_ms(&self, ms: u32) {
        Timer::after(Duration::from_millis(ms as u64)).await;
    }
}

struct Observer;
impl<H> LinkObserver<H> for Observer {
    fn link_down(&mut self) {
        info!("SETUP: station link down, reconnecting");
    }
    fn ready(&mut self, _network: H) {
        info!("SETUP: station READY");
    }
}

/// Answers the form's requests until dropped; `serve_forever_io` never returns by itself.
async fn serve_page<L: ConnectionListener>(listener: &mut L) -> ! {
    let router = HttpRouter::new()
        .route(
            "/",
            get(|| async { Response::ok(PAGE).with_content_type("text/html; charset=utf-8") }),
        )
        .route(
            "/networks",
            get(|| async {
                let scanning = SCANNING.lock(|c| *c.borrow());
                let text = NETWORKS.lock(|list| networks_text(&list.borrow(), scanning));
                Response::ok(text).with_content_type("text/plain; charset=utf-8")
            }),
        )
        .route(
            "/scan",
            get(|| async {
                SCANNING.lock(|c| *c.borrow_mut() = true);
                SCAN_REQUEST.signal(());
                Response::ok("ok").with_content_type("text/plain; charset=utf-8")
            }),
        )
        .route(
            "/status",
            get(|| async {
                Response::ok(status_text(&status())).with_content_type("text/plain; charset=utf-8")
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
    serve_forever_io(listener, &router).await
}

/// Handles scan and connect requests until one provisioning succeeds and its answer has been
/// readable for a few seconds.
async fn provisioning_loop<T, B>(wifi: &mut WifiManager<T, B>)
where
    T: WifiTransport,
    T::Address: Display,
    B: ConfigBackend,
    B::Error: Debug,
{
    loop {
        let (ssid, password) = match select(REQUEST.wait(), SCAN_REQUEST.wait()).await {
            Either::First(credentials) => credentials,
            Either::Second(()) => {
                refresh_networks(wifi).await;
                continue;
            }
        };
        info!(
            "SETUP: t={}ms provisioning requested (ssid {} bytes), access point stays up",
            now_ms(),
            ssid.len()
        );
        // Let the HTTP answer leave first: joining the network moves the radio to the router's
        // channel, and the access point follows it.
        Timer::after_millis(800).await;
        if wifi.provision(&ssid, password).await {
            let address = wifi
                .ip()
                .map(|a| alloc::format!("{a}"))
                .unwrap_or_else(|| String::from("?"));
            info!(
                "SETUP: t={}ms provisioned, station connected, ip={address}",
                now_ms()
            );
            set_status(Status::Connected(address));
            // Let the page's status poll read the result before its network disappears.
            Timer::after_secs(8).await;
            return;
        }
        set_status(Status::Failed);
        warn!(
            "SETUP: t={}ms connection failed; access point stays up for a retry",
            now_ms()
        );
    }
}

/// The unconfigured path: scan, open the access point, serve the form, provision with the access
/// point still up, then close the page and stop the access point. Returns once credentials are
/// saved and the station is connected.
async fn setup<T, B, P>(
    wifi: &mut WifiManager<T, B>,
    access_point: &AccessPointConfig,
    page: &mut P,
) where
    T: WifiTransport + WifiAccessPoint,
    T::Address: Display,
    B: ConfigBackend,
    B::Error: Debug,
    P: PageListener<<T as WifiAccessPoint>::NetworkHandle>,
{
    // First scan before the access point exists: nobody is connected yet, so nothing is disturbed.
    refresh_networks(wifi).await;
    if !wifi.start_access_point(access_point).await {
        warn!("SETUP: FAIL access point did not start");
        core::future::pending::<()>().await;
    }
    let Some(handle) = wifi.access_point_handle() else {
        warn!("SETUP: FAIL no access point handle");
        core::future::pending::<()>().await;
        return;
    };
    info!(
        "SETUP: t={}ms access point ACTIVE: join \"{}\" and open the page",
        now_ms(),
        access_point.ssid()
    );
    {
        let mut listener = page.listener(handle);
        // The page ends with the sequence: leaving this block closes its listener before the
        // access point goes away (the product revokes its own service).
        select(serve_page(&mut listener), provisioning_loop(wifi)).await;
    }
    info!(
        "SETUP: t={}ms page stopped, stopping access point (the radio restarts)",
        now_ms()
    );
    wifi.stop_access_point().await;
    info!(
        "SETUP: t={}ms access point stopped, handing over to maintain",
        now_ms()
    );
}

/// The product: boots into the network when credentials are saved, otherwise provisions through
/// the access point; then keeps the station connected forever.
pub async fn run<T, B, P>(
    transport: T,
    space: ConfigSpace<B>,
    access_point: &AccessPointConfig,
    mut page: P,
) -> !
where
    T: WifiTransport + WifiAccessPoint,
    T::Address: Display,
    B: ConfigBackend,
    B::Error: Debug,
    P: PageListener<<T as WifiAccessPoint>::NetworkHandle>,
{
    let mut provisioned = is_provisioned(&space).await;
    let mut wifi = WifiManager::new(transport, space);
    info!(
        "SETUP: t={}ms start, saved credentials: {}",
        now_ms(),
        if provisioned { "yes" } else { "no" }
    );
    loop {
        if !provisioned {
            setup(&mut wifi, access_point, &mut page).await;
        } else {
            info!("SETUP: saved credentials found, joining without the access point");
        }
        // The saved credentials drive the station, at every boot and after a setup.
        let ended = wifi.maintain(&EmbassySleep, &mut Observer).await;
        warn!("SETUP: maintain ended ({ended:?}): no usable credentials, back to setup");
        provisioned = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(ssid: &str, signal: i8, secured: bool) -> Network {
        Network {
            ssid: String::from(ssid),
            signal_strength: signal,
            secured,
        }
    }

    #[test]
    fn networks_are_listed_strongest_first() {
        let ranked = rank(std::vec![
            net("far", -80, true),
            net("near", -40, true),
            net("mid", -60, false)
        ]);
        let names: std::vec::Vec<&str> = ranked.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(names, ["near", "mid", "far"]);
    }

    #[test]
    fn the_list_has_a_state_line_then_one_line_per_network() {
        let text = networks_text(&[net("Maison", -45, true), net("Café", -67, false)], false);
        assert_eq!(text, "ready\n-45\t1\tMaison\n-67\t0\tCaf%C3%A9");
        assert_eq!(networks_text(&[], true), "scanning");
    }

    #[test]
    fn ssid_bytes_cannot_break_the_line_structure() {
        let text = networks_text(&[net("a\tb\nc%d <img onerror=x>", -50, true)], false);
        let lines: std::vec::Vec<&str> = text.split('\n').collect();
        assert_eq!(lines.len(), 2, "a newline in an SSID must not add a line");
        let fields: std::vec::Vec<&str> = lines[1].split('\t').collect();
        assert_eq!(fields.len(), 3, "a tab in an SSID must not add a field");
        assert!(!fields[2].contains(['<', '>', ' ', '=']));
        assert_eq!(fields[2], "a%09b%0Ac%25d%20%3Cimg%20onerror%3Dx%3E");
    }

    #[test]
    fn status_texts_start_with_the_words_the_page_polls_for() {
        assert!(status_text(&Status::Connecting).starts_with("connecting"));
        assert!(status_text(&Status::Idle).starts_with("en attente"));
        assert_eq!(
            status_text(&Status::Connected(String::from("192.168.1.9"))),
            "connecté, adresse 192.168.1.9"
        );
        assert!(status_text(&Status::Failed).starts_with("échec"));
    }
}
