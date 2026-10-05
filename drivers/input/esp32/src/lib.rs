#![no_std]
use esp_hal::{
    gpio::{Input, InputConfig, Pull},
    peripherals::GPIO0,
};
/// Reference profile BOOT input. Debounce/hold/recovery are product policy.
/// GPIO0 held low at reset selects the ROM downloader, before this code runs.
pub fn boot_button(pin: GPIO0<'static>) -> Input<'static> {
    Input::new(pin, InputConfig::default().with_pull(Pull::Up))
}
