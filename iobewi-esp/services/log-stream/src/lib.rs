#![no_std]

//! ESP port for the portable log stream: the RNG its WebSocket layer needs.
//! (Console output is `iobewi-esp-console`; the secure outbound connection is
//! the generic `iobewi_esp_tls::service::EspClientTransport`.)

#[cfg(feature = "esp32s3")]
mod esp {
    use esp_hal::rng::Rng;
    use iobewi_log_stream::Entropy;

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
pub use esp::EspLogEntropy;
