#![no_std]
#![no_main]
use iobewi_board::{Board, BootIoFactory, ResourceRequest, SerialBank, UsbBootMode};
use iobewi_config_space::{Budget, ConfigManager};
use iobewi_device::DeviceMetadata;
pub const BOARD_RESOURCES: ResourceRequest = ResourceRequest {
    sockets: 3,
    heap_bytes: 96 * 1024,
    minimum_stack_bytes: 16 * 1024,
};
entry_api::entry!(crate::run);
// Actual product policy is deliberately absent: this compile fixture loads an
// opaque space and chooses provisioning. It does not implement StreamBeWI's flag.
async fn run<B: Board>(board: B) {
    let parts = board.into_parts();
    let mut config = ConfigManager::new(parts.config);
    let space = config.claim("board_fixture", Budget::new(16)).ok().unwrap();
    let loaded = space.load().await;
    let mut io = parts
        .io
        .select(if cfg!(feature = "mass-storage") {
            UsbBootMode::MassStorage
        } else {
            UsbBootMode::Provisioning
        })
        .ok()
        .unwrap();
    while let Some(port) = io.serial.take_next() {
        core::hint::black_box(port);
    }
    core::hint::black_box(parts.identity.chip_name());
    core::hint::black_box((&parts.wifi, &parts.button, &parts.reset, &io.usb, &loaded));
    core::future::pending::<()>().await;
    core::hint::black_box((&parts.wifi, &parts.button, &parts.reset, &io.usb, &loaded));
}
