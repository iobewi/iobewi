#![no_std]

use iobewi_entropy::EntropySource;

/// ESP hardware RNG implementation of IOBEWI's portable entropy capability.
#[derive(Clone, Copy, Default)]
pub struct EspEntropySource;

impl EntropySource for EspEntropySource {
    fn fill_random(&self, output: &mut [u8]) {
        esp_hal::rng::Rng::new().read(output);
    }
}
