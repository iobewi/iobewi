#![no_std]
#![no_main]

//! ESP32-S3 composition of `wifi-ap-setup-example`: the only place that names the chip. It owns
//! the HAL, heap, RTOS, the single flash owner, the NVS config space, the Wi-Fi transport with
//! its access point and the page's TCP listener, then hands everything to the portable `run`.

extern crate alloc;

use embassy_executor::Spawner;
use embassy_net::{Stack, StackResources};
use iobewi_config_space::ConfigManager;
use iobewi_esp_config_space::NvsConfigBackend;
use iobewi_esp_tcp::EspTcpListener;
use iobewi_esp_wifi::WifiManager as EspWifi;
use iobewi_wifi_core::AccessPointConfig;
use iobewi_wifi_manager::CONFIG_BUDGET;
use static_cell::StaticCell;
use wifi_ap_setup_example::PageListener;

esp_bootloader_esp_idf::esp_app_desc!();

/// Station sockets as in a product; the access point needs the DHCP server (1 UDP) and the
/// page's listener (1 TCP) plus margin.
const STA_SOCKETS: usize = 4;
const AP_SOCKETS: usize = 4;
static STA: StaticCell<StackResources<STA_SOCKETS>> = StaticCell::new();
static AP: StaticCell<StackResources<AP_SOCKETS>> = StaticCell::new();

const SETUP_SSID: &str = "IOBEWI-Setup";
/// Test-only passphrase, overridable at build time (`SETUP_AP_PASSWORD`). A product must use a
/// unique secret (ADR-0017).
const SETUP_PASSPHRASE: &str = match option_env!("SETUP_AP_PASSWORD") {
    Some(value) => value,
    None => "123456789",
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // No message: a panic payload could carry configuration text.
    esp_println::println!("SETUP: panic");
    loop {
        core::hint::spin_loop();
    }
}

/// The page's listener over the access point's embassy-net stack, port 80.
struct EspPage {
    rx: [u8; 1024],
    tx: [u8; 1024],
}

impl PageListener<Stack<'static>> for EspPage {
    type Listener<'a> = EspTcpListener<'a>;

    fn listener(&mut self, access_point_network: Stack<'static>) -> EspTcpListener<'_> {
        EspTcpListener::new(access_point_network, 80, &mut self.rx, &mut self.tx)
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: 160 * 1024);
    let timer = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timer.timer0, peripherals.FROM_CPU_INTR0);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    // The single physical flash owner, then the repository's NVS config-space backend on the
    // discovered "nvs" partition (no address is hardcoded).
    let flash = iobewi_esp_flash::init(peripherals.FLASH);
    let Ok(backend) = NvsConfigBackend::from_label(flash, "nvs").await else {
        esp_println::println!("SETUP: FAIL configuration storage (NVS partition)");
        core::future::pending::<()>().await;
        return;
    };
    let space = ConfigManager::new(backend)
        .claim("wifi", CONFIG_BUDGET)
        .unwrap();

    let transport = EspWifi::new(peripherals.WIFI, spawner, STA.init(StackResources::new()))
        .with_access_point(AP.init(StackResources::new()));
    let access_point = AccessPointConfig::new(SETUP_SSID, SETUP_PASSPHRASE, 6).unwrap();
    let page = EspPage { rx: [0; 1024], tx: [0; 1024] };

    wifi_ap_setup_example::run(transport, space, &access_point, page).await
}
