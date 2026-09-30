#![no_std]

extern crate alloc;

use alloc::string::String;

/// Platform capability exposing a stable hardware-derived identifier.
///
/// The returned identifier is opaque to the application: callers may use it
/// as a suffix or persistence key, but must not assume it is a MAC address,
/// eFuse value, serial number, or any other platform-specific representation.
pub trait DeviceIdentity {
    fn hardware_id(&self) -> String;
}

/// Static facts about the device/platform that applications may report.
///
/// These are hardware/platform facts, not application policy. OTA-specific
/// facts such as the partition-layout contract live in `iobewi-ota`.
pub trait DeviceMetadata {
    fn chip_name(&self) -> &'static str;
    fn ram_size(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;

    impl DeviceIdentity for Fixed {
        fn hardware_id(&self) -> String {
            String::from("abc123")
        }
    }

    impl DeviceMetadata for Fixed {
        fn chip_name(&self) -> &'static str {
            "test-chip"
        }

        fn ram_size(&self) -> u32 {
            123_456
        }
    }

    #[test]
    fn capabilities_report_implementation_values_verbatim() {
        let device = Fixed;
        assert_eq!(device.hardware_id(), "abc123");
        assert_eq!(device.chip_name(), "test-chip");
        assert_eq!(device.ram_size(), 123_456);
    }
}
