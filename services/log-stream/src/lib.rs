#![no_std]

//! Platform ports for the portable IOBEWI log streaming service.

extern crate alloc;

#[cfg(feature = "esp32s3")]
mod esp {
    use alloc::{format, string::String};
    use core::ffi::CStr;
    use embassy_net::Stack;
    use esp_hal::rng::Rng;
    use iobewi_esp_tls::mbedtls_rs::SessionError;
    use iobewi_esp_tls::service::{self as tls, ClientStream, TlsConfigSpace, TlsReferenceStatic};
    use iobewi_log_stream::Transport;

    /// ESP console output; the portable service owns filtering and buffering.
    pub fn install(application_target: &'static str) {
        iobewi_log_stream::install(|record: &log::Record<'_>| {
            esp_println::println!("{} - {}", record.level(), record.args());
        }, application_target);
    }

    #[derive(Clone, Copy)]
    pub struct EspLogTransport {
        pub stack: Stack<'static>,
        pub tls: TlsReferenceStatic,
        pub tls_config: &'static TlsConfigSpace,
        pub clock_is_set: fn() -> bool,
    }

    impl Transport for EspLogTransport {
        type IoError = SessionError;
        type Connection<'host, 'buffers> = ClientStream<'host, 'buffers>;

        async fn connect<'host, 'buffers>(
            &self,
            host: &'host CStr,
            port: u16,
            rx: &'buffers mut [u8],
            tx: &'buffers mut [u8],
        ) -> Result<Self::Connection<'host, 'buffers>, String> {
            tls::connect_client(self.tls, self.stack, self.tls_config, (self.clock_is_set)(), rx, tx, host, port)
                .await.map_err(|error| format!("connect to {host:?}:{port} failed: {error}"))
        }

        fn random_bytes(&self, output: &mut [u8]) {
            Rng::new().read(output);
        }

        fn random_u32(&self) -> u32 {
            Rng::new().random()
        }
    }
}

#[cfg(feature = "esp32s3")]
pub use esp::{install, EspLogTransport};
