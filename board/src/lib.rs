#![no_std]

//! Owned board capabilities for a product composed in a single firmware binary.
//!
//! This contract initializes no hardware and defines no persisted schema.
//! Platform adapters supply the resources; the product owns provisioning,
//! recovery, service orchestration and its additional capability bounds.
//! It does not cross an independently built Workload ABI.

use embedded_io_async::{Read, Write};

/// Independently owned receive and transmit halves of one serial port.
///
/// Implementations must coordinate any shared hardware internally. Console and
/// protocol writers must not independently acquire the same peripheral.
pub struct Serial<R, W> {
    pub rx: R,
    pub tx: W,
}

/// A finite, owned list of serial ports configured by the selected board mode.
///
/// Each call transfers the next port's ownership to the caller. Once exhausted,
/// every subsequent call must return `None`: no replenishment or reacquisition.
/// One RX/TX representation may use adapter enums for heterogeneous peripherals.
/// The product must not assume a fixed pair, order or physical port names.
pub trait SerialBank {
    type Rx: Read + 'static;
    type Tx: Write + 'static;

    fn take_next(&mut self) -> Option<Serial<Self::Rx, Self::Tx>>;
}

/// Native USB mode selected once during boot, before either controller is created.
///
/// The product maps its own persisted configuration to this mode. This enum does
/// not imply a flag schema, an automatic transition or an immediate reboot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbBootMode {
    /// Serial/JTAG may be initialized; OTG must not be initialized.
    Provisioning,
    /// OTG is initialized; Serial/JTAG must not be initialized or returned.
    MassStorage,
}

/// Serial resources and an optional USB device driver for the selected boot mode.
///
/// Successful Provisioning returns `usb: None`; successful MassStorage returns
/// `Some(driver)`. Independent UART ports may remain available in either mode.
/// These are adapter obligations, not runtime hardware checks in this crate.
pub struct BootIo<S, D> {
    pub serial: S,
    pub usb: Option<D>,
}

/// An owned factory consumed before normal product services start.
///
/// `select` is synchronous: it constructs the selected resources without waiting
/// for a host or performing a running USB handover. Unsupported modes and
/// construction failures return an explicit error. On failure, an adapter must
/// not leave both native USB controllers active; cleanup belongs to the adapter.
/// Neither construction nor console/panic paths may bypass exclusive ownership.
/// Factories owning unique hardware must not implement `Clone` or `Copy`.
///
/// A value cannot be used for two selections:
/// ```compile_fail,E0382
/// use iobewi_board::{BootIoFactory, UsbBootMode};
/// fn select_twice<F: BootIoFactory>(factory: F) {
///     let _ = factory.select(UsbBootMode::Provisioning);
///     let _ = factory.select(UsbBootMode::MassStorage);
/// }
/// ```
pub trait BootIoFactory: Sized {
    type Serial: SerialBank;
    // This is the same trait reexported by embassy_usb::driver, without the stack.
    type Driver: embassy_usb_driver::Driver<'static>;
    type Error: core::fmt::Debug;

    fn select(self, mode: UsbBootMode) -> Result<BootIo<Self::Serial, Self::Driver>, Self::Error>;
}

/// Owned reset capability; reset scope and implementation belong to the platform.
///
/// The product decides when to reset, after its persistence operations complete.
/// An ESP adapter must qualify USB routing cleanup after its chosen reset scope.
pub trait Reset {
    fn reset(self) -> !;
}

/// Independent product capabilities transferred by [`Board::into_parts`].
///
/// `io` is still unselected: the product can read `config` before initializing
/// either native USB controller. A configuration handle may share a backend;
/// that sharing must not create an additional physical flash owner.
pub struct BoardParts<W, C, B, IO, R, I> {
    pub wifi: W,
    pub config: C,
    pub button: B,
    pub io: IO,
    pub reset: R,
    pub identity: I,
}

/// A same-binary, statically dispatched set of owned platform capabilities.
///
/// Implementations construct hardware once and must not duplicate unique owners
/// through `Clone`/`Copy`, globals or a second startup path. The consuming signature
/// protects one value; Rust cannot prevent an adapter from duplicating its own
/// underlying hardware. NVS discovery and SharedFlash ownership remain platform
/// obligations. This trait exposes no HAL, GPIO, concrete network stack or pins.
///
/// Products may add associated-type bounds required by their services, including
/// a specific network handle. Board itself keeps `NetworkHandle` opaque.
///
/// A value cannot be split twice:
/// ```compile_fail,E0382
/// use iobewi_board::Board;
/// fn split_twice<B: Board>(board: B) {
///     let _ = board.into_parts();
///     let _ = board.into_parts();
/// }
/// ```
pub trait Board: Sized {
    type Wifi: iobewi_wifi_core::WifiTransport;
    type Config: iobewi_config_space::ConfigBackend;
    type Button: embedded_hal_async::digital::Wait;
    type Io: BootIoFactory;
    type Reset: Reset;
    type Identity: iobewi_device::DeviceIdentity + iobewi_device::DeviceMetadata;

    fn into_parts(
        self,
    ) -> BoardParts<Self::Wifi, Self::Config, Self::Button, Self::Io, Self::Reset, Self::Identity>;
}
