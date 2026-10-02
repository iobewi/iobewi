#![no_std]

extern crate alloc;

use alloc::string::String;
use iobewi_device::{hardware_id_from_mac, DeviceIdentity, DeviceMetadata};

/// ESP implementation of IOBEWI's portable device identity capability.
///
/// Reads the eFuse-burned base MAC address; the identifier convention (last
/// three bytes, lowercase hex, no prefix) is `iobewi_device::hardware_id_from_mac`.
#[derive(Clone, Copy, Default)]
pub struct EspDeviceIdentity;

impl DeviceIdentity for EspDeviceIdentity {
    fn hardware_id(&self) -> String {
        let mac = esp_hal::efuse::base_mac_address();
        let bytes = mac.as_bytes();
        hardware_id_from_mac([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]])
    }

    fn mac_address(&self) -> Option<[u8; 6]> {
        let mac = esp_hal::efuse::base_mac_address();
        let bytes = mac.as_bytes();
        Some([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]])
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
