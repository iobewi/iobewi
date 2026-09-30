#![no_std]

//! Platform ports for the portable IOBEWI log streaming service.

extern crate alloc;

#[cfg(feature = "esp32s3")]
mod esp {
    use embassy_net::Stack;
    use esp_hal::rng::Rng;
    use iobewi_esp_tls::service::{self as tls, ClientStream, ClientTlsError, ConfigBackend, TlsConfigSpace, TlsReferenceStatic};
    use iobewi_log_stream::Entropy;
    use iobewi_transport::SecureClientTransport;

    /// ESP console output passed to the portable service at composition time.
    pub fn console_print(record: &log::Record<'_>) {
        esp_println::println!("{} - {}", record.level(), record.args());
    }

    #[derive(Clone, Copy)]
    pub struct EspLogTransport<B: ConfigBackend + 'static> {
        pub stack: Stack<'static>,
        pub tls: TlsReferenceStatic,
        pub tls_config: &'static TlsConfigSpace<B>,
        pub clock_is_set: fn() -> bool,
    }

    impl<B> SecureClientTransport for EspLogTransport<B>
    where
        B: ConfigBackend + 'static,
        B::Error: core::fmt::Debug,
    {
        type Error = ClientTlsError;
        type Connection<'a>
            = ClientStream<'a>
        where
            Self: 'a;

        async fn connect<'a>(
            &'a self,
            host: &'a str,
            port: u16,
            rx: &'a mut [u8],
            tx: &'a mut [u8],
        ) -> Result<Self::Connection<'a>, Self::Error> {
            tls::connect_client(self.tls, self.stack, self.tls_config, (self.clock_is_set)(), rx, tx, host, port).await
        }
    }

    impl<B: ConfigBackend + 'static> Entropy for EspLogTransport<B> {
        fn random_bytes(&self, output: &mut [u8]) {
            Rng::new().read(output);
        }

        fn random_u32(&self) -> u32 {
            Rng::new().random()
        }
    }
}

#[cfg(feature = "esp32s3")]
pub use esp::{console_print, EspLogTransport};
