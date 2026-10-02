#![no_std]

//! ESP RMT/WS2812 renderer for the portable IOBEWI status indicator
//! capability.
//!
//! | Status       | Colour | Pattern              |
//! |--------------|--------|----------------------|
//! | `Booting`    | white  | steady               |
//! | `Ready`      | blue   | slow blink           |
//! | `Scanning`   | blue   | fast blink           |
//! | `Connecting` | orange | fast blink           |
//! | `Online`     | green  | steady               |
//! | `Failed`     | red    | blink, until retried |

use core::sync::atomic::{AtomicU8, Ordering};

use embassy_time::{Duration, Timer};
use esp_hal::gpio::AnyPin;
use esp_hal::peripherals::RMT;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal_smartled::{RmtSmartLeds, Timing, buffer_size, color_order};
use iobewi_indicator::{Status, StatusIndicator, StatusIndicatorCapabilities};
use log::{info, warn};
use smart_leds::{RGB8, SmartLedsWrite};

/// `esp-hal-smartled2` 0.29.0 multiplies its pulse widths by an extra `* 2`,
/// assuming the RMT counter runs at twice the given source clock. That is
/// wrong here, and made the LED sit at full-brightness white whatever was
/// written (upstream issue #9, reported for the ESP32-S3). Pre-halving every
/// duration cancels that doubling.
const WS2812_TIMING_HALVED: Timing = Timing {
    time_0_high: esp_hal_smartled::WS2812_TIMING.time_0_high / 2,
    time_0_low: esp_hal_smartled::WS2812_TIMING.time_0_low / 2,
    time_1_high: esp_hal_smartled::WS2812_TIMING.time_1_high / 2,
    time_1_low: esp_hal_smartled::WS2812_TIMING.time_1_low / 2,
    reset: esp_hal_smartled::WS2812_TIMING.reset / 2,
};

fn to_byte(status: Status) -> u8 {
    match status {
        Status::Booting => 0,
        Status::Ready => 1,
        Status::Scanning => 2,
        Status::Connecting => 3,
        Status::Online => 4,
        Status::Failed => 5,
    }
}

fn from_byte(byte: u8) -> Status {
    match byte {
        1 => Status::Ready,
        2 => Status::Scanning,
        3 => Status::Connecting,
        4 => Status::Online,
        5 => Status::Failed,
        _ => Status::Booting,
    }
}

/// Colour, and how long each on/off phase lasts (`None` stays lit).
fn pattern(status: Status) -> (RGB8, Option<Duration>) {
    let fast = Some(Duration::from_millis(150));
    match status {
        Status::Booting => (RGB8::new(10, 10, 10), None),
        Status::Ready => (RGB8::new(0, 0, 30), Some(Duration::from_millis(500))),
        Status::Scanning => (RGB8::new(0, 0, 30), fast),
        Status::Connecting => (RGB8::new(30, 15, 0), fast),
        Status::Online => (RGB8::new(0, 20, 0), None),
        Status::Failed => (RGB8::new(30, 0, 0), Some(Duration::from_millis(300))),
    }
}

/// riscv32imc has no atomic read-modify-write, but a plain store is enough:
/// the value is only ever overwritten, never updated in place.
static STATE: AtomicU8 = AtomicU8::new(0);

/// ESP implementation of the portable [`StatusIndicator`] capability.
/// Zero-sized: the actual state lives in a process-wide static, shared with
/// [`led_task`] -- there is only ever one status LED on this device.
#[derive(Clone, Copy, Default)]
pub struct EspStatusIndicator;

/// GPIOs this ESP32-S3 firmware accepts for the status LED: every GPIO the
/// SoC exposes except the ones a board cannot use freely -- GPIO22..25 do not
/// exist on the S3, GPIO26..32 are the SPI flash, and GPIO33..37 are taken by
/// octal flash/PSRAM on modules that have it. That leaves 0..=21 and 38..=48
/// (GPIO47 is the onboard LED of several S3 boards). A board/firmware choice,
/// not chip-universal: a C3 build needs its own list before
/// `EspStatusIndicator` can support it (feature-gated for that reason).
#[cfg(feature = "esp32s3")]
pub const STATUS_LED_GPIO_NUMBERS: &[u8] = &[
    0, 1, 2, 3, 4, 5, 6, 7,
    8, 9, 10, 11, 12, 13, 14, 15,
    16, 17, 18, 19, 20, 21,
    38, 39, 40, 41, 42, 43, 44, 45,
    46, 47, 48,
];

#[cfg(feature = "esp32s3")]
impl StatusIndicatorCapabilities for EspStatusIndicator {
    fn configurable_pins(&self) -> &'static [u8] {
        STATUS_LED_GPIO_NUMBERS
    }
}

impl StatusIndicator for EspStatusIndicator {
    fn set(&self, status: Status) {
        STATE.store(to_byte(status), Ordering::Relaxed);
    }
}

async fn park() -> ! {
    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}

/// Drives the LED to match the last [`EspStatusIndicator::set`] call. Never
/// panics: without the LED the rest of the firmware still works, so a driver
/// failure only costs the display. Only spawn this when a status LED GPIO is
/// actually configured.
#[embassy_executor::task]
pub async fn led_task(rmt: RMT<'static>, pin: AnyPin<'static>) -> ! {
    let rmt = match Rmt::new(rmt, Rate::from_mhz(80)) {
        Ok(rmt) => rmt,
        Err(e) => {
            warn!("Status LED unavailable, RMT init failed: {e:?}");
            park().await
        }
    };
    let mut led = match RmtSmartLeds::<
        { buffer_size::<RGB8>(1) },
        _,
        RGB8,
        color_order::Rgb,
    >::new_with_memsize(WS2812_TIMING_HALVED, rmt.channel0, pin, 2)
    {
        Ok(led) => led,
        Err(e) => {
            warn!("Status LED unavailable, WS2812 init failed: {e:?}");
            park().await
        }
    };

    info!("Status LED: RMT + WS2812 driver ready");
    let mut shown: Option<RGB8> = None;
    let mut lit = true;
    loop {
        let (colour, phase) = pattern(from_byte(STATE.load(Ordering::Relaxed)));
        let colour = if phase.is_some() && !lit {
            RGB8::default()
        } else {
            colour
        };
        if shown != Some(colour) {
            if let Err(e) = led.write([colour].into_iter()) {
                warn!("LED write failed: {e:?}");
            }
            shown = Some(colour);
        }

        lit = !lit;
        Timer::after(phase.unwrap_or(Duration::from_millis(200))).await;
    }
}
