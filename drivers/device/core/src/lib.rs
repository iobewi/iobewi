#![no_std]

extern crate alloc;

use alloc::string::String;

/// The framework's convention for a MAC-derived hardware identifier: the last
/// three bytes of the base MAC address, lowercase hexadecimal, zero-padded, with
/// no application prefix (what Embewi has always used for `embewi-<id>` names).
///
/// This is the *identity* rule; reading the MAC (eFuse, NIC, ...) is the
/// platform's job, so a platform implementation only supplies the six bytes.
pub fn hardware_id_from_mac(mac: [u8; 6]) -> String {
    alloc::format!("{:02x}{:02x}{:02x}", mac[3], mac[4], mac[5])
}

/// Platform capability exposing a stable hardware-derived identifier.
///
/// The returned identifier is opaque to the application: callers may use it
/// as a suffix or persistence key, but must not assume it is a MAC address,
/// eFuse value, serial number, or any other platform-specific representation.
pub trait DeviceIdentity {
    fn hardware_id(&self) -> String;

    /// Link-layer MAC address when the platform exposes one.
    ///
    /// Not every device has a MAC, so callers must treat this as an
    /// optional capability rather than deriving platform identity from it.
    fn mac_address(&self) -> Option<[u8; 6]> {
        None
    }
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

        fn mac_address(&self) -> Option<[u8; 6]> {
            Some([0, 1, 2, 3, 4, 5])
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
    fn hardware_id_is_the_last_three_mac_bytes_lowercase_and_zero_padded() {
        assert_eq!(
            hardware_id_from_mac([0xaa, 0xbb, 0xcc, 0x0a, 0x0b, 0xff]),
            "0a0bff"
        );
        assert_eq!(hardware_id_from_mac([0, 0, 0, 0, 0, 0]), "000000");
        assert_eq!(
            hardware_id_from_mac([0x7c, 0xdf, 0xa1, 0xac, 0x4e, 0x8c]),
            "ac4e8c"
        );
    }

    #[test]
    fn capabilities_report_implementation_values_verbatim() {
        let device = Fixed;
        assert_eq!(device.hardware_id(), "abc123");
        assert_eq!(device.mac_address(), Some([0, 1, 2, 3, 4, 5]));
        assert_eq!(device.chip_name(), "test-chip");
        assert_eq!(device.ram_size(), 123_456);
    }
}

/// Adapter-local GPIO identifier, distinct from a physical package/header pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinId(pub u16);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalDirection {
    Input,
    Output,
}
/// Read-only alternate mux function from the platform's metadata source.
/// Names/selectors are opaque display metadata, not a portable register API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinFunction {
    pub signal: &'static str,
    pub selector: &'static str,
    pub direction: SignalDirection,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinDescriptor {
    pub id: PinId,
    pub name: &'static str,
    pub digital_input: bool,
    pub digital_output: bool,
    pub functions: &'static [PinFunction],
}
/// Read-only MCU GPIO capabilities, separate from board wiring/availability.
/// This grants no peripheral ownership and does not certify an unused/exposed pin.
/// Identifiers may be sparse; consumers must look up by id rather than index.
pub trait PinMetadata {
    fn pins(&self) -> &'static [PinDescriptor];
    fn pin(&self, id: PinId) -> Option<&'static PinDescriptor> {
        self.pins().iter().find(|pin| pin.id == id)
    }
}
#[cfg(test)]
mod pin_tests {
    use super::*;
    struct Fake;
    impl PinMetadata for Fake {
        fn pins(&self) -> &'static [PinDescriptor] {
            &[PinDescriptor {
                id: PinId(3),
                name: "PA3",
                digital_input: true,
                digital_output: false,
                functions: &[],
            }]
        }
    }
    #[test]
    fn sparse_adapter_ids_are_not_slice_indices_or_header_pin_numbers() {
        assert_eq!(Fake.pin(PinId(3)).unwrap().name, "PA3");
        assert!(Fake.pin(PinId(0)).is_none());
        assert!(!Fake.pin(PinId(3)).unwrap().digital_output);
    }
}
