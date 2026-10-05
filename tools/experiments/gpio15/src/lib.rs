//! Host proof for the production GPIO projection, without HAL or registers.
pub struct EspDeviceMetadata;
mod pins {
    include!("../../../../drivers/device/esp32/src/pins.rs");
}
#[cfg(test)]
mod tests {
    use super::*;
    use iobewi_device::{PinId, PinMetadata, SignalDirection};
    #[test]
    fn projection_preserves_sparse_gpio_inventory_and_uart_alternates() {
        let metadata = EspDeviceMetadata;
        assert_eq!(metadata.pins().len(), 45);
        assert!(metadata.pin(PinId(22)).is_none());
        assert!(metadata.pin(PinId(48)).is_some());
        let tx = metadata.pin(PinId(43)).unwrap();
        assert_eq!(tx.name, "GPIO43");
        assert!(tx.digital_input && tx.digital_output);
        assert!(
            tx.functions
                .iter()
                .any(|f| f.signal == "U0TXD" && f.direction == SignalDirection::Output)
        );
        assert!(
            metadata
                .pin(PinId(44))
                .unwrap()
                .functions
                .iter()
                .any(|f| f.signal == "U0RXD" && f.direction == SignalDirection::Input)
        );
    }
}
