#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_net::{Ipv4Address, Ipv4Cidr, Runner, StackResources, StaticConfigV4};
use embassy_net::{Stack, tcp::TcpSocket};
use embassy_time::{Duration, Timer, with_timeout};
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
    with_timeout(Duration::from_secs(20), controller.connect_async())
        .await
        .unwrap()
        .unwrap();
    with_timeout(Duration::from_secs(20), station.wait_config_up())
        .await
        .unwrap();
    println!(
        "AP31: station ready ip={:?}; attach the single-connection TCP probe now",
        station.config_v4().map(|c| c.address.address())
    );
    Timer::after_secs(30).await;
    for cycle in 0..3 {
        println!(
            "AP31: before cycle={} connected={} sta_link={} sta_ip={} heap={}",
            cycle,
            controller.is_connected(),
            station.is_link_up(),
            station.is_config_up(),
            esp_alloc::HEAP.free()
        );
        controller
            .set_config(&Config::AccessPointStation(sta.clone(), ap.clone()))
            .unwrap();
        #[cfg(not(feature = "dormant-apsta"))]
        {
            // Observe loss of the OLD DHCP configuration before reconnecting.
            with_timeout(Duration::from_secs(20), station.wait_config_down())
                .await
                .unwrap();
            with_timeout(Duration::from_secs(20), controller.connect_async())
                .await
                .unwrap()
                .unwrap();
            with_timeout(Duration::from_secs(20), station.wait_config_up())
                .await
                .unwrap();
            println!("AP31: recovery after mode change (not keep-link)");
        }
        Timer::after_secs(2).await;
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
        #[cfg(feature = "dormant-apsta")]
        controller
            .set_config(&Config::AccessPointStation(sta.clone(), dormant.clone()))
            .unwrap();
        #[cfg(not(feature = "dormant-apsta"))]
        {
            controller
                .set_config(&Config::Station(sta.clone()))
                .unwrap();
            with_timeout(Duration::from_secs(20), station.wait_config_down())
                .await
                .unwrap();
            with_timeout(Duration::from_secs(20), controller.connect_async())
                .await
                .unwrap()
                .unwrap();
            with_timeout(Duration::from_secs(20), station.wait_config_up())
                .await
                .unwrap();
        }
        Timer::after_secs(2).await;
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
    #[cfg(feature = "dormant-apsta")]
    println!("AP31: done; AP dormant, radio still active (not AP stopped)");
    #[cfg(not(feature = "dormant-apsta"))]
    println!("AP31: done; AP disabled");
    core::future::pending::<()>().await;
}
