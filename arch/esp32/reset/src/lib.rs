#![no_std]

//! Physical reset of the ESP32 SoC. Two primitives, deliberately kept apart
//! because they reset different scopes:
//!
//! * [`software_reset`] -- an immediate "digital core" reset. Fast and
//!   dependency-free, but it leaves the native USB-Serial-JTAG peripheral's
//!   link state untouched.
//! * [`arm_system_reset`] -- arms the RTC watchdog with the broadest reset
//!   scope (`ResetSystem`) so the chip resets itself after a short delay. This
//!   one *does* reset USB-Serial-JTAG, which is what a freshly booted firmware
//!   (and a host tool talking Improv Serial) expects.
//!
//! No Embassy, no HTTP, no firmware policy: both may be called from degraded or
//! very low-level code. *When* to reset (after a graceful shutdown, after an
//! HTTP response has reached its socket, after a rollback decision) is the
//! caller's business; the hardware watchdog used as a boot-window guard is
//! `iobewi-esp-watchdog`, a different peripheral.

use esp_hal::peripherals::RTC_TIMER;
use esp_hal::rtc_cntl::{Rtc, RwdtStage, RwdtStageAction};

/// Immediate digital-core reset. Never returns.
pub fn software_reset() -> ! {
    esp_hal::system::software_reset()
}

/// Arm the RTC watchdog so the whole system resets after `after_ms`.
///
/// Returns the `Rtc` handle: the caller keeps it alive (and then simply waits
/// for the reset), exactly as the watchdog is configured and enabled here.
pub fn arm_system_reset(rtc_timer: RTC_TIMER<'static>, after_ms: u64) -> Rtc<'static> {
    let mut rtc = Rtc::new(rtc_timer);
    rtc.rwdt
        .set_timeout(RwdtStage::Stage0, esp_hal::time::Duration::from_millis(after_ms));
    rtc.rwdt.set_stage_action(RwdtStage::Stage0, RwdtStageAction::ResetSystem);
    rtc.rwdt.enable();
    rtc
}
