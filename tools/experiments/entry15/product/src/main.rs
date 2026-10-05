#![no_std]
#![no_main]
extern crate alloc;
use alloc::boxed::Box;
use entry_api::{Board, SharedFlashAccess};

// Exercise collisions with the previous macro's root imports and item names.
use entry_api::esp_hal;
const _: fn() -> esp_hal::clock::CpuClock = esp_hal::clock::CpuClock::max;
static ENTRY15_CHIP: u8 = 1;
static ENTRY15_MAIN_LAYOUT: u8 = 2;
static ENTRY15_PRODUCT_LAYOUT: u8 = 3;

entry_api::entry!(crate::run);

async fn run<B: Board>(board: B) {
    let mut parts = board.into_parts();
    let mut allocation = Box::new([parts.flash.read_marker(); 128]);
    core::hint::black_box((&ENTRY15_CHIP, &ENTRY15_MAIN_LAYOUT, &ENTRY15_PRODUCT_LAYOUT));
    core::hint::black_box((&mut parts.buffer, &mut allocation, &mut parts.flash));
    core::future::pending::<()>().await;
    core::hint::black_box((&mut parts.buffer, &mut allocation, &mut parts.flash));
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
