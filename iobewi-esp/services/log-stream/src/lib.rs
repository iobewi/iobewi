#![no_std]

//! Platform ports for the portable IOBEWI log streaming service: console
//! output and the RNG the WebSocket layer needs. The secure outbound
//! connection itself is the generic `iobewi_esp_tls::service::EspClientTransport`.

extern crate alloc;

#[cfg(feature = "esp32s3")]
mod esp {
    use esp_hal::rng::Rng;
    use iobewi_log_stream::Entropy;

    /// ESP console output passed to the portable service at composition time.
    pub fn console_print(record: &log::Record<'_>) {
        esp_println::println!("{} - {}", record.level(), record.args());
    }

    /// ESP hardware RNG for the log stream's WebSocket nonce and frame masking.
    #[derive(Clone, Copy, Default)]
    pub struct EspLogEntropy;

    impl Entropy for EspLogEntropy {
        fn random_bytes(&self, output: &mut [u8]) {
            Rng::new().read(output);
        }

        fn random_u32(&self) -> u32 {
            Rng::new().random()
        }
    }
}

#[cfg(feature = "esp32s3")]
pub use esp::{console_print, EspLogEntropy};
