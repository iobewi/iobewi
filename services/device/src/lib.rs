#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use iobewi_device::{DeviceIdentity, DeviceMetadata};

/// ESP implementation of IOBEWI's portable device identity capability.
///
/// The stable hardware identifier intentionally preserves the convention
/// already used by Embewi: the last three bytes of the eFuse-burned base
/// MAC address, lowercase hexadecimal, with no application prefix.
#[derive(Clone, Copy, Default)]
pub struct EspDeviceIdentity;

impl DeviceIdentity for EspDeviceIdentity {
    fn hardware_id(&self) -> String {
        let mac = esp_hal::efuse::base_mac_address();
        let bytes = mac.as_bytes();
        format!("{:02x}{:02x}{:02x}", bytes[3], bytes[4], bytes[5])
    }

    fn mac_address(&self) -> Option<[u8; 6]> {
        let mac = esp_hal::efuse::base_mac_address();
        Some(*mac.as_bytes())
    }
}

/// ESP implementation of static IOBEWI device metadata.
#[derive(Clone, Copy, Default)]
pub struct EspDeviceMetadata;

impl DeviceMetadata for EspDeviceMetadata {
    fn chip_name(&self) -> &'static str {
        esp_metadata_generated::chip_pretty!()
    }

    fn ram_size(&self) -> u32 {
        let dram = esp_metadata_generated::memory_range!("DRAM");
        (dram.end - dram.start) as u32
    }
}
