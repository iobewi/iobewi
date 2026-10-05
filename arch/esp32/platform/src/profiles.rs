//! Board wiring and resource limits, distinct from SoC mux/interrupt tables.
/// The first wiring profile matches StreamBeWI's inspected S3 target (9869f4f).
/// It describes connected peripherals, not every legal MCU pin or packaged board.
#[derive(Debug, Clone, Copy)]
pub struct BoardProfile {
    pub button_pin: u8,
    pub button_active_low: bool,
    pub uart_tx: u8,
    pub uart_rx: u8,
    pub usb_dp: u8,
    pub usb_dm: u8,
    pub nvs_label: &'static str,
    /// Conservative composition budget for heap, network resources and OTG OUT.
    /// Not a measurement of free RAM or a socket count imposed on the product.
    pub resource_budget_bytes: usize,
}
pub const S3_NATIVE_USB: BoardProfile = BoardProfile {
    button_pin: 0,
    button_active_low: true,
    uart_tx: 43,
    uart_rx: 44,
    usb_dp: 20,
    usb_dm: 19,
    nvs_label: "nvs",
    resource_budget_bytes: 256 * 1024,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceError {
    NoHeap,
    NoStack,
    Overflow,
    OverBudget,
}
/// Product requests are admitted by memory, never silently reduced.
pub fn validate_resources(
    heap: usize,
    socket_storage: usize,
    usb_out: usize,
    stack_min: usize,
    budget: usize,
) -> Result<(), ResourceError> {
    if heap == 0 {
        return Err(ResourceError::NoHeap);
    }
    if stack_min == 0 {
        return Err(ResourceError::NoStack);
    }
    let total = heap
        .checked_add(socket_storage)
        .and_then(|n| n.checked_add(usb_out))
        .and_then(|n| n.checked_add(stack_min))
        .ok_or(ResourceError::Overflow)?;
    if total > budget {
        return Err(ResourceError::OverBudget);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_matches_reference_wiring() {
        assert_eq!(
            (
                S3_NATIVE_USB.button_pin,
                S3_NATIVE_USB.uart_tx,
                S3_NATIVE_USB.uart_rx,
                S3_NATIVE_USB.usb_dp,
                S3_NATIVE_USB.usb_dm
            ),
            (0, 43, 44, 20, 19)
        );
        assert!(S3_NATIVE_USB.button_active_low);
        assert_eq!(S3_NATIVE_USB.nvs_label, "nvs");
    }
    #[test]
    fn resource_admission_is_checked_without_socket_count_policy() {
        assert_eq!(validate_resources(98304, 1024, 1024, 16384, 262144), Ok(()));
        assert_eq!(
            validate_resources(0, 0, 0, 1, 100),
            Err(ResourceError::NoHeap)
        );
        assert_eq!(
            validate_resources(1, 0, 0, 0, 100),
            Err(ResourceError::NoStack)
        );
        assert_eq!(
            validate_resources(99, 1, 1, 1, 100),
            Err(ResourceError::OverBudget)
        );
        assert_eq!(
            validate_resources(usize::MAX, 1, 0, 1, usize::MAX),
            Err(ResourceError::Overflow)
        );
    }
}
