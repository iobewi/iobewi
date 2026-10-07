#![no_std]
#![no_main]

use core::future::Future;
use embassy_executor::Spawner;
use embassy_net::{Ipv4Address, Ipv4Cidr, Runner, StackResources, StaticConfigV4};
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use embedded_io_async::Write;
use esp_println::println;
use esp_radio::wifi::{
    AuthenticationMethodConfig, Config, ControllerConfig, Interface, WifiController,
    ap::AccessPointConfig, sta::StationConfig,
};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();
static STA: StaticCell<StackResources<4>> = StaticCell::new();
static AP: StaticCell<StackResources<3>> = StaticCell::new();

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    println!("AP31: panic (details intentionally omitted)");
    loop {
        core::hint::spin_loop();
    }
}

#[embassy_executor::task(pool_size = 2)]
async fn network(mut runner: Runner<'static, Interface>) -> ! {
    runner.run().await
}

// Raw TCP echo on STA: no HTTP/admin route on the station network.
#[embassy_executor::task]
async fn station_echo(stack: Stack<'static>) {
    let mut rx = [0; 1024];
    let mut tx = [0; 1024];
    let mut packet = [0; 256];
    loop {
        let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
        socket.set_timeout(Some(Duration::from_secs(5)));
        if socket.accept(3131).await.is_err() {
            continue;
        }
        println!("AP31: TCP accepted");
        loop {
            match socket.read(&mut packet).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if socket.write_all(&packet[..n]).await.is_err() {
                        break;
                    }
                }
            }
        }
        println!("AP31: TCP ended");
        socket.abort();
    }
}

// Isolated AP-only test page; no credential endpoint or admin router.
#[embassy_executor::task]
async fn ap_page(stack: Stack<'static>) {
    let mut rx = [0; 1024];
    let mut tx = [0; 1024];
    let mut request = [0; 128];
    loop {
        stack.wait_link_up().await;
        let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
        socket.set_timeout(Some(Duration::from_secs(3)));
        if socket.accept(80).await.is_err() {
            continue;
        }
        if let Ok(n) = socket.read(&mut request).await {
            let response: &[u8] = if request[..n].starts_with(b"GET / HTTP/1.") {
                b"HTTP/1.1 200 OK\r\nContent-Length: 16\r\nConnection: close\r\n\r\nAP31 experiment\n"
            } else {
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            };
            let _ = socket.write_all(response).await;
            let _ = with_timeout(Duration::from_secs(3), socket.flush()).await;
        }
        socket.abort();
    }
}

const VARIANT: &str = if cfg!(feature = "dormant-apsta") {
    "dormant-apsta"
} else {
    "mode-transition"
};

/// Phase marker with the device uptime, to line UART up with an external
/// sniffer/client capture. `cycle` is -1 outside the cycle loop.
fn phase(cycle: i8, name: &str) {
    println!(
        "AP31: t={}ms variant={} cycle={} phase={}",
        Instant::now().as_millis(),
        VARIANT,
        cycle,
        name
    );
}

/// Diagnostic stop: marker only (no error value, SSID or secret), then suspend
/// WITHOUT resetting. This does not restore the radio; it keeps the failing
/// state observable (the TCP echo and AP page tasks keep running) so the UART
/// capture and the external probe can still be attributed.
async fn halt(cycle: i8, name: &str) -> ! {
    println!(
        "AP31: FAIL t={}ms variant={} cycle={} phase={}; experiment suspended, no reset. Save the UART capture before any reset.",
        Instant::now().as_millis(),
        VARIANT,
        cycle,
        name
    );
    core::future::pending::<()>().await;
    unreachable!()
}

async fn must<T, E>(cycle: i8, name: &str, result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => halt(cycle, name).await,
    }
}

async fn timed<T>(cycle: i8, name: &str, seconds: u64, future: impl Future<Output = T>) -> T {
    match with_timeout(Duration::from_secs(seconds), future).await {
        Ok(value) => value,
        Err(_) => halt(cycle, name).await,
    }
}

/// Which internal esp-radio branch ran. Only available with the patched copy of
/// esp-radio (`run.sh MODE VARIANT counters`); counts, never configuration values.
#[cfg(feature = "radio-counters")]
fn diag(cycle: i8, after: &str) {
    use core::sync::atomic::Ordering::Relaxed;
    use esp_radio::wifi::ap31_diag::*;
    println!(
        "AP31: diag cycle={} after={} sta_skipped={} sta_applied={} ap_skipped={} ap_applied={} radio_stops={}",
        cycle,
        after,
        STA_SKIPPED.load(Relaxed),
        STA_APPLIED.load(Relaxed),
        AP_SKIPPED.load(Relaxed),
        AP_APPLIED.load(Relaxed),
        RADIO_STOPS.load(Relaxed)
    );
}

#[cfg(not(feature = "radio-counters"))]
fn diag(cycle: i8, after: &str) {
    println!(
        "AP31: diag cycle={} after={} counters=unavailable(official esp-radio)",
        cycle, after
    );
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: 128 * 1024);
    let timer = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timer.timer0, peripherals.FROM_CPU_INTR0);

    // Credentials exist only in this local qualification binary, never in logs.
    // Required variables prevent accidentally exposing an open AP.
    let sta = StationConfig::default()
        .with_ssid(env!("AP31_STA_SSID").try_into().unwrap())
        .with_authentication(AuthenticationMethodConfig::Wpa2Personal(
            env!("AP31_STA_PASSWORD").try_into().unwrap(),
        ));
    let ap = AccessPointConfig::default()
        .with_ssid("IOBEWI-AP31-TEST".try_into().unwrap())
        .with_authentication(AuthenticationMethodConfig::Wpa2Personal(
            env!("AP31_AP_PASSWORD").try_into().unwrap(),
        ))
        .with_channel(1)
        .with_max_connections(2);
    #[cfg(feature = "dormant-apsta")]
    let dormant = {
        let secret = env!("AP31_DORMANT_PASSWORD");
        assert!(secret != env!("AP31_AP_PASSWORD"));
        AccessPointConfig::default()
            .with_ssid("IOBEWI-AP31-DORMANT".try_into().unwrap())
            .with_ssid_hidden(true)
            .with_authentication(AuthenticationMethodConfig::Wpa2Personal(
                secret.try_into().unwrap(),
            ))
            .with_channel(1)
            .with_max_connections(1)
    };
    let mut controller = WifiController::new(
        peripherals.WIFI,
        ControllerConfig::default().with_initial_config({
            #[cfg(feature = "dormant-apsta")]
            {
                Config::AccessPointStation(sta.clone(), dormant.clone())
            }
            #[cfg(not(feature = "dormant-apsta"))]
            {
                Config::Station(sta.clone())
            }
        }),
    )
    .unwrap();
    let (station, station_runner) = embassy_net::new(
        Interface::station(),
        embassy_net::Config::dhcpv4(Default::default()),
        STA.init(StackResources::new()),
        31,
    );
    let (access_point, ap_runner) = embassy_net::new(
        Interface::access_point(),
        embassy_net::Config::ipv4_static(StaticConfigV4 {
            address: Ipv4Cidr::new(Ipv4Address::new(172, 23, 241, 1), 24),
            gateway: None,
            dns_servers: Default::default(),
        }),
        AP.init(StackResources::new()),
        32,
    );
    spawner.spawn(network(station_runner).unwrap());
    spawner.spawn(network(ap_runner).unwrap());
    spawner.spawn(station_echo(station).unwrap());
    spawner.spawn(ap_page(access_point).unwrap());
    phase(-1, "initial-connect");
    must(
        -1,
        "initial-connect",
        timed(-1, "initial-connect", 20, controller.connect_async()).await,
    )
    .await;
    timed(-1, "initial-dhcp", 20, station.wait_config_up()).await;
    diag(-1, "initial-connected");
    println!(
        "AP31: station ready ip={:?}; attach the single-connection TCP probe now",
        station.config_v4().map(|c| c.address.address())
    );
    Timer::after_secs(30).await;
    for cycle in 0..3i8 {
        println!(
            "AP31: before cycle={} connected={} sta_link={} sta_ip={} heap={}",
            cycle,
            controller.is_connected(),
            station.is_link_up(),
            station.is_config_up(),
            esp_alloc::HEAP.free()
        );
        phase(cycle, "request-active");
        must(
            cycle,
            "request-active",
            controller.set_config(&Config::AccessPointStation(sta.clone(), ap.clone())),
        )
        .await;
        diag(cycle, "request-active");
        #[cfg(not(feature = "dormant-apsta"))]
        {
            // Observe loss of the OLD DHCP configuration before reconnecting.
            timed(cycle, "active-config-down", 20, station.wait_config_down()).await;
            must(
                cycle,
                "active-reconnect",
                timed(cycle, "active-reconnect", 20, controller.connect_async()).await,
            )
            .await;
            timed(cycle, "active-dhcp", 20, station.wait_config_up()).await;
            println!("AP31: recovery after mode change (not keep-link)");
        }
        Timer::after_secs(2).await;
        phase(cycle, "active");
        println!(
            "AP31: active connected={} sta_link={} sta_ip={} ap_link={} ap_ip={} sta_channel={:?} heap={}",
            controller.is_connected(),
            station.is_link_up(),
            station.is_config_up(),
            access_point.is_link_up(),
            access_point.is_config_up(),
            controller.ap_info().ok().map(|i| i.channel),
            esp_alloc::HEAP.free()
        );
        // In the dormant variant NEVER reconnect: the external probe must
        // observe any disruption instead of having it silently repaired.
        Timer::after_secs(30).await;
        phase(cycle, "request-inactive");
        #[cfg(feature = "dormant-apsta")]
        must(
            cycle,
            "request-inactive",
            controller.set_config(&Config::AccessPointStation(sta.clone(), dormant.clone())),
        )
        .await;
        #[cfg(not(feature = "dormant-apsta"))]
        {
            must(
                cycle,
                "request-inactive",
                controller.set_config(&Config::Station(sta.clone())),
            )
            .await;
            diag(cycle, "request-inactive");
            timed(
                cycle,
                "inactive-config-down",
                20,
                station.wait_config_down(),
            )
            .await;
            must(
                cycle,
                "inactive-reconnect",
                timed(cycle, "inactive-reconnect", 20, controller.connect_async()).await,
            )
            .await;
            timed(cycle, "inactive-dhcp", 20, station.wait_config_up()).await;
        }
        #[cfg(feature = "dormant-apsta")]
        diag(cycle, "request-inactive");
        Timer::after_secs(2).await;
        phase(cycle, "inactive");
        println!(
            "AP31: inactive-requested connected={} sta_link={} sta_ip={} ap_link={} sta_channel={:?} heap={}",
            controller.is_connected(),
            station.is_link_up(),
            station.is_config_up(),
            access_point.is_link_up(),
            controller.ap_info().ok().map(|i| i.channel),
            esp_alloc::HEAP.free()
        );
        // Leave a client associated across this change. Check whether it can
        // still reach the AP page; hidden SSID/rotated credentials do not
        // prove that existing clients were deauthenticated.
        Timer::after_secs(30).await;
    }
    phase(3, "done");
    #[cfg(feature = "dormant-apsta")]
    println!("AP31: done; AP dormant, radio still active (not AP stopped)");
    #[cfg(not(feature = "dormant-apsta"))]
    println!("AP31: done; AP disabled");
    core::future::pending::<()>().await;
}
