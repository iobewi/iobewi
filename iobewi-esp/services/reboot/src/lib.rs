#![no_std]

//! One-shot ESP reboot adapter for the portable IOBEWI OTA reboot
//! capability (`iobewi_ota_http::RebootPort`).
//!
//! Owns exactly one thing: turning "schedule a reboot" into a real RTC
//! watchdog reset, deferred long enough for an in-flight HTTP response to
//! actually reach the socket. Knows nothing about OTA, HTTP routes,
//! provisioning, the agent, tokens, or lifecycle state -- those stay
//! entirely on the application side of `RebootPort`.

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::peripherals::LPWR;
use esp_hal::rtc_cntl::{Rtc, RwdtStage, RwdtStageAction};
use iobewi_ota_http::RebootPort;
use static_cell::StaticCell;

type LpwrCell = Mutex<CriticalSectionRawMutex, Option<LPWR<'static>>>;

/// Gives the response time to actually reach the socket before resetting --
/// calling a reset directly from the request handler would cut the
/// connection before picoserve ever writes the confirmation page.
///
/// Uses the RTC watchdog (`ResetSystem`, the broadest of the three reset
/// scopes esp-hal exposes) instead of `esp_hal::system::software_reset()`.
/// That function only does a "digital core" reset, which on this chip
/// leaves the native USB-Serial-JTAG peripheral's link state untouched: the
/// host still sees the old USB session, the freshly-booted firmware expects
/// a new one, and Improv Serial stops responding correctly until a real
/// (EN-pin/RTS-triggered) reset -- exactly what ESP Web Tools itself always
/// does when it resets the board, which is why that path never showed this.
#[embassy_executor::task]
async fn reboot_after_delay(lpwr: LPWR<'static>) -> ! {
    Timer::after(Duration::from_millis(500)).await;
    let mut rtc = Rtc::new(lpwr);
    rtc.rwdt
        .set_timeout(RwdtStage::Stage0, esp_hal::time::Duration::from_millis(100));
    rtc.rwdt.set_stage_action(RwdtStage::Stage0, RwdtStageAction::ResetSystem);
    rtc.rwdt.enable();
    loop {
        Timer::after(Duration::from_secs(10)).await;
    }
}

/// ESP implementation of the portable [`RebootPort`] capability. Holds the
/// one-shot `LPWR` peripheral behind a lock so this handle can be `Clone`d
/// across every route that might request a reboot (`/reboot`,
/// `/ota/activate`, the provisioning form, ...), while only ever actually
/// consuming the peripheral -- and therefore only ever actually
/// rebooting -- once: whichever caller's `schedule_reboot()` runs first
/// takes it, every later call (concurrent or not) is a no-op.
#[derive(Clone)]
pub struct EspReboot {
    cell: &'static LpwrCell,
    spawner: Spawner,
}

impl EspReboot {
    /// Construct once, at the composition root, from the one physical
    /// `LPWR` peripheral. Calling this more than once panics (the internal
    /// storage is a process-lifetime static, initialized exactly once).
    pub fn new(lpwr: LPWR<'static>, spawner: Spawner) -> Self {
        static CELL: StaticCell<LpwrCell> = StaticCell::new();
        let cell = CELL.init(Mutex::new(Some(lpwr)));
        Self { cell, spawner }
    }
}

impl RebootPort for EspReboot {
    async fn schedule_reboot(&self) {
        if let Some(lpwr) = self.cell.lock().await.take()
            && let Ok(spawn_token) = reboot_after_delay(lpwr)
        {
            self.spawner.spawn(spawn_token);
        }
    }
}
