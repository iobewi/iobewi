mod support;

use core::future::Future;
use core::task::{Context, Poll};
use embedded_hal_async::digital::Wait;
use embedded_io_async::{Read, Write};
use iobewi_board::*;
use iobewi_config_space::{Budget, ConfigBackend, ConfigManager};
use iobewi_device::{DeviceIdentity, DeviceMetadata};
use iobewi_wifi_core::WifiTransport;
use std::sync::Arc;
use std::task::Waker;
use support::*;

fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("this fake operation must complete in one poll"),
    }
}
fn ids<S: SerialBank<Rx = Rx, Tx = Tx>>(bank: &mut S) -> Vec<u8> {
    let mut result = Vec::new();
    while let Some(port) = bank.take_next() {
        result.push(port.rx.id);
    }
    result
}
fn board() -> (FakeBoard, Arc<Constructors>) {
    let constructors = Arc::new(Constructors::default());
    (
        FakeBoard {
            constructors: constructors.clone(),
            supports_otg: true,
        },
        constructors,
    )
}

#[test]
fn zero_ports_stays_exhausted() {
    let mut bank = Bank::new([]);
    for _ in 0..4 {
        assert!(bank.take_next().is_none());
    }
}

#[test]
fn three_ports_transfer_independent_halves_and_stay_exhausted() {
    let mut bank = Bank::new([7, 11, 42]);
    let mut ports = Vec::new();
    while let Some(port) = bank.take_next() {
        ports.push(port);
    }
    assert_eq!(ports.len(), 3);
    for _ in 0..4 {
        assert!(bank.take_next().is_none());
    }
    // The bank is gone; all transferred halves remain usable independently.
    drop(bank);
    for (port, expected) in ports.iter_mut().zip([7, 11, 42]) {
        let mut byte = [0];
        assert_eq!(ready(port.rx.read(&mut byte)).unwrap(), 1);
        assert_eq!(byte[0], expected);
        assert_eq!(port.tx.id, expected);
        assert_eq!(ready(port.tx.write(&[expected, 99])).unwrap(), 2);
        ready(port.tx.flush()).unwrap();
        assert_eq!(port.tx.written, [expected, 99]);
    }
    // RX and TX can be moved to distinct service owners.
    let Serial { mut rx, mut tx } = ports.remove(0);
    drop(ports);
    assert_eq!(ready(rx.read(&mut [0])).unwrap(), 1);
    ready(tx.write(&[5])).unwrap();
    assert_eq!(tx.written, [7, 99, 5]);
}

#[test]
fn splitting_board_defers_usb_until_product_selects_provisioning() {
    let (board, constructors) = board();
    assert_eq!(constructors.counts(), (0, 0, 0));
    let parts = board.into_parts();
    assert_eq!(constructors.counts(), (1, 0, 0));
    let mut io = parts.io.select(UsbBootMode::Provisioning).unwrap();
    assert_eq!(constructors.counts(), (1, 1, 0));
    assert!(io.usb.is_none());
    assert_eq!(ids(&mut io.serial), [0, 1, 2]);
}

#[test]
fn mass_storage_never_constructs_or_returns_jtag_and_preserves_uarts() {
    let (board, constructors) = board();
    let parts = board.into_parts();
    let mut io = parts.io.select(UsbBootMode::MassStorage).unwrap();
    assert_eq!(constructors.counts(), (1, 0, 1));
    assert!(io.usb.is_some());
    assert_eq!(ids(&mut io.serial), [0, 2]);
    assert!(io.serial.take_next().is_none());
}

#[test]
fn unsupported_selection_is_an_explicit_error_without_usb_construction() {
    let (mut board, constructors) = board();
    board.supports_otg = false;
    let result = board.into_parts().io.select(UsbBootMode::MassStorage);
    assert!(matches!(result, Err(SelectionError::Unsupported)));
    assert_eq!(constructors.counts(), (1, 0, 0));
}

// Product-specific bound on an opaque handle, not an Embassy stack imposed by Board.
fn network<B: Board>(board: B) -> B::Wifi
where
    B::Wifi: WifiTransport<NetworkHandle = OpaqueNetwork>,
{
    board.into_parts().wifi
}
#[test]
fn generic_product_adds_its_own_network_handle_bound() {
    let (board, _) = board();
    let mut wifi = network(board);
    assert!(wifi.network_handle().is_none());
    assert!(ready(wifi.connect("test", "secret".into())));
    assert_eq!(wifi.network_handle(), Some(OpaqueNetwork(7)));
    assert!(wifi.ip().is_some());
}

#[test]
fn split_capabilities_supply_configuration_and_both_identity_traits() {
    let (board, constructors) = board();
    let parts = board.into_parts();
    let mut manager = ConfigManager::new(parts.config.clone());
    let space = manager.claim("product-test", Budget::new(16)).unwrap();
    ready(space.commit(b"opaque-value")).unwrap();
    let snapshot = ready(parts.config.load("product-test")).unwrap().unwrap();
    assert_eq!(snapshot.data, b"opaque-value");
    assert_eq!(parts.identity.hardware_id(), "fake-id");
    assert_eq!(parts.identity.mac_address(), Some([0, 1, 2, 3, 4, 5]));
    assert_eq!(parts.identity.chip_name(), "fake-chip");
    assert_eq!(parts.identity.ram_size(), 123_456);
    // Persistence and identity inspection happen before native USB construction.
    assert_eq!(constructors.counts(), (1, 0, 0));
}

#[test]
fn unavailable_button_wait_remains_pending() {
    let (board, _) = board();
    let mut button = board.into_parts().button;
    let mut wait = std::pin::pin!(button.wait_for_any_edge());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(wait.as_mut().poll(&mut cx).is_pending());
    assert!(wait.as_mut().poll(&mut cx).is_pending());
}
