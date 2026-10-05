#![no_std]
#[cfg(not(feature = "esp32s3"))]
compile_error!("iobewi-entry requires the supported esp32s3 target feature");
#[cfg(all(feature = "esp32s3", not(target_arch = "xtensa")))]
compile_error!("iobewi-entry esp32s3 requires target xtensa-esp32s3-none-elf");
#[cfg(feature = "esp32s3")]
mod imp;
#[cfg(feature = "esp32s3")]
#[doc(hidden)]
pub use imp::*;
pub use iobewi_board::ResourceRequest;
