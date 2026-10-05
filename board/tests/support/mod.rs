#![allow(async_fn_in_trait)]

use core::convert::Infallible;
use core::future::pending;
use embassy_usb_driver as usb;
use embedded_io_async::{ErrorType, Read, Write};
use iobewi_board::*;
use iobewi_config_space::{Budget, ConfigBackend, Snapshot};
use iobewi_device::{DeviceIdentity, DeviceMetadata};
use iobewi_wifi_core::{Network, WifiTransport};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct Rx {
    pub id: u8,
}
pub struct Tx {
    pub id: u8,
    pub written: Vec<u8>,
}
impl ErrorType for Rx {
    type Error = Infallible;
}
impl ErrorType for Tx {
    type Error = Infallible;
}
impl Read for Rx {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Infallible> {
        if let Some(first) = buf.first_mut() {
            *first = self.id;
            Ok(1)
        } else {
            Ok(0)
        }
    }
}
impl Write for Tx {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }
    async fn flush(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}
pub struct Bank {
    ports: VecDeque<Serial<Rx, Tx>>,
}
impl Bank {
    pub fn new(ids: impl IntoIterator<Item = u8>) -> Self {
        Self {
            ports: ids
                .into_iter()
                .map(|id| Serial {
                    rx: Rx { id },
                    tx: Tx {
                        id,
                        written: Vec::new(),
                    },
                })
                .collect(),
        }
    }
}
impl SerialBank for Bank {
    type Rx = Rx;
    type Tx = Tx;
    fn take_next(&mut self) -> Option<Serial<Rx, Tx>> {
        self.ports.pop_front()
    }
}

// These fakes model constructor selection, not the USB protocol. Endpoint/bus
// types are uninhabited: no USB stack is started by these host tests.
pub enum Never {}
impl usb::Endpoint for Never {
    fn info(&self) -> &usb::EndpointInfo {
        match *self {}
    }
    async fn wait_enabled(&mut self) {
        match *self {}
    }
}
impl usb::EndpointOut for Never {
    async fn read(&mut self, _: &mut [u8]) -> Result<usize, usb::EndpointError> {
        match *self {}
    }
}
impl usb::EndpointIn for Never {
    async fn write(&mut self, _: &[u8]) -> Result<(), usb::EndpointError> {
        match *self {}
    }
}
impl usb::Bus for Never {
    async fn enable(&mut self) {
        match *self {}
    }
    async fn disable(&mut self) {
        match *self {}
    }
    async fn poll(&mut self) -> usb::Event {
        match *self {}
    }
    fn endpoint_set_enabled(&mut self, _: usb::EndpointAddress, _: bool) {
        match *self {}
    }
    fn endpoint_set_stalled(&mut self, _: usb::EndpointAddress, _: bool) {
        match *self {}
    }
    fn endpoint_is_stalled(&mut self, _: usb::EndpointAddress) -> bool {
        match *self {}
    }
    async fn remote_wakeup(&mut self) -> Result<(), usb::Unsupported> {
        match *self {}
    }
}
impl usb::ControlPipe for Never {
    fn max_packet_size(&self) -> usize {
        match *self {}
    }
    async fn setup(&mut self) -> [u8; 8] {
        match *self {}
    }
    async fn data_out(
        &mut self,
        _: &mut [u8],
        _: bool,
        _: bool,
    ) -> Result<usize, usb::EndpointError> {
        match *self {}
    }
    async fn data_in(&mut self, _: &[u8], _: bool, _: bool) -> Result<(), usb::EndpointError> {
        match *self {}
    }
    async fn accept(&mut self) {
        match *self {}
    }
    async fn reject(&mut self) {
        match *self {}
    }
    async fn accept_set_address(&mut self, _: u8) {
        match *self {}
    }
}
pub struct UsbToken;
impl<'a> usb::Driver<'a> for UsbToken {
    type EndpointOut = Never;
    type EndpointIn = Never;
    type ControlPipe = Never;
    type Bus = Never;
    fn alloc_endpoint_out(
        &mut self,
        _: usb::EndpointType,
        _: Option<usb::EndpointAddress>,
        _: u16,
        _: u8,
    ) -> Result<Never, usb::EndpointAllocError> {
        Err(usb::EndpointAllocError)
    }
    fn alloc_endpoint_in(
        &mut self,
        _: usb::EndpointType,
        _: Option<usb::EndpointAddress>,
        _: u16,
        _: u8,
    ) -> Result<Never, usb::EndpointAllocError> {
        Err(usb::EndpointAllocError)
    }
    fn start(self, _: u16) -> (Never, Never) {
        panic!("host fixture does not implement USB protocol")
    }
}
#[derive(Default)]
pub struct Constructors {
    pub splits: AtomicUsize,
    pub jtag: AtomicUsize,
    pub otg: AtomicUsize,
}
impl Constructors {
    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.splits.load(Ordering::SeqCst),
            self.jtag.load(Ordering::SeqCst),
            self.otg.load(Ordering::SeqCst),
        )
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum SelectionError {
    Unsupported,
}
pub struct Factory {
    pub constructors: Arc<Constructors>,
    pub supports_otg: bool,
}
impl BootIoFactory for Factory {
    type Serial = Bank;
    type Driver = UsbToken;
    type Error = SelectionError;
    fn select(self, mode: UsbBootMode) -> Result<BootIo<Bank, UsbToken>, SelectionError> {
        match mode {
            UsbBootMode::Provisioning => {
                self.constructors.jtag.fetch_add(1, Ordering::SeqCst);
                // Fake IDs: UART0, Serial/JTAG, auxiliary UART.
                Ok(BootIo {
                    serial: Bank::new([0, 1, 2]),
                    usb: None,
                })
            }
            UsbBootMode::MassStorage if self.supports_otg => {
                self.constructors.otg.fetch_add(1, Ordering::SeqCst);
                Ok(BootIo {
                    serial: Bank::new([0, 2]),
                    usb: Some(UsbToken),
                })
            }
            UsbBootMode::MassStorage => Err(SelectionError::Unsupported),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpaqueNetwork(pub u8);
// Address deliberately has no Display bound; Board must not require one.
pub struct Address;
pub struct Wifi {
    pub online: bool,
}
impl WifiTransport for Wifi {
    type Address = Address;
    type NetworkHandle = OpaqueNetwork;
    async fn connect(&mut self, _: &str, _: String) -> bool {
        self.online = true;
        true
    }
    async fn scan(&mut self) -> Vec<Network> {
        Vec::new()
    }
    async fn wait_down(&mut self) {
        if self.online {
            pending::<()>().await;
        }
    }
    fn ip(&self) -> Option<Address> {
        self.online.then_some(Address)
    }
    fn network_handle(&self) -> Option<OpaqueNetwork> {
        self.online.then_some(OpaqueNetwork(7))
    }
    fn is_online(&self) -> bool {
        self.online
    }
}
#[derive(Clone, Default)]
pub struct Config(Arc<Mutex<BTreeMap<String, Snapshot>>>);
impl ConfigBackend for Config {
    type Error = Infallible;
    fn capacity_units(&self) -> usize {
        4096
    }
    fn reservation_units(&self, _: &str, budget: Budget) -> Option<usize> {
        Some(budget.max_bytes())
    }
    async fn load(&self, space: &str) -> Result<Option<Snapshot>, Infallible> {
        Ok(self.0.lock().unwrap().get(space).cloned())
    }
    async fn commit(&self, space: &str, data: &[u8]) -> Result<u64, Infallible> {
        let mut values = self.0.lock().unwrap();
        let generation = values.get(space).map_or(1, |s| s.generation + 1);
        values.insert(
            space.into(),
            Snapshot {
                generation,
                data: data.into(),
            },
        );
        Ok(generation)
    }
    async fn clear(&self, space: &str) -> Result<u64, Infallible> {
        let previous = self.0.lock().unwrap().remove(space);
        Ok(previous.map_or(1, |s| s.generation + 1))
    }
}
pub struct Button;
impl embedded_hal::digital::ErrorType for Button {
    type Error = Infallible;
}
impl embedded_hal_async::digital::Wait for Button {
    async fn wait_for_high(&mut self) -> Result<(), Infallible> {
        pending().await
    }
    async fn wait_for_low(&mut self) -> Result<(), Infallible> {
        pending().await
    }
    async fn wait_for_rising_edge(&mut self) -> Result<(), Infallible> {
        pending().await
    }
    async fn wait_for_falling_edge(&mut self) -> Result<(), Infallible> {
        pending().await
    }
    async fn wait_for_any_edge(&mut self) -> Result<(), Infallible> {
        pending().await
    }
}
pub struct ResetToken;
impl Reset for ResetToken {
    fn reset(self) -> ! {
        panic!("host fixture cannot reset hardware")
    }
}
pub struct Identity;
impl DeviceIdentity for Identity {
    fn hardware_id(&self) -> String {
        "fake-id".into()
    }
    fn mac_address(&self) -> Option<[u8; 6]> {
        Some([0, 1, 2, 3, 4, 5])
    }
}
impl DeviceMetadata for Identity {
    fn chip_name(&self) -> &'static str {
        "fake-chip"
    }
    fn ram_size(&self) -> u32 {
        123_456
    }
}
pub struct FakeBoard {
    pub constructors: Arc<Constructors>,
    pub supports_otg: bool,
}
impl Board for FakeBoard {
    type Wifi = Wifi;
    type Config = Config;
    type Button = Button;
    type Io = Factory;
    type Reset = ResetToken;
    type Identity = Identity;
    fn into_parts(self) -> BoardParts<Wifi, Config, Button, Factory, ResetToken, Identity> {
        self.constructors.splits.fetch_add(1, Ordering::SeqCst);
        BoardParts {
            wifi: Wifi { online: false },
            config: Config::default(),
            button: Button,
            io: Factory {
                constructors: self.constructors,
                supports_otg: self.supports_otg,
            },
            reset: ResetToken,
            identity: Identity,
        }
    }
}
