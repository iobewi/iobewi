use super::EspDeviceMetadata;
use iobewi_device::{PinDescriptor, PinFunction, PinId, PinMetadata, SignalDirection};
// Build directly from the pinned esp-hal metadata macros: no copied SoC tables.
macro_rules! has_attribute {
    ($wanted:ident;) => { false };
    (Input; [Input] $($tail:tt)*) => { true };
    (Output; [Output] $($tail:tt)*) => { true };
    ($wanted:ident; [$other:ident] $($tail:tt)*) => { has_attribute!($wanted; $($tail)*) };
}
esp_metadata_generated::for_each_gpio! {
    (all $(($n:literal, $gpio:ident ($($in_selector:ident => $input:ident)*) ($($out_selector:ident => $output:ident)*) ($($attributes:tt)*))),*) => {
        pub static PINS: &[PinDescriptor] = &[$(
            PinDescriptor {
                id: PinId($n), name: stringify!($gpio),
                digital_input: has_attribute!(Input; $($attributes)*),
                digital_output: has_attribute!(Output; $($attributes)*),
                functions: &[
                    $(PinFunction { signal: stringify!($input), selector: stringify!($in_selector), direction: SignalDirection::Input },)*
                    $(PinFunction { signal: stringify!($output), selector: stringify!($out_selector), direction: SignalDirection::Output },)*
                ],
            },
        )*];
    };
}
impl PinMetadata for EspDeviceMetadata {
    fn pins(&self) -> &'static [PinDescriptor] {
        PINS
    }
}
