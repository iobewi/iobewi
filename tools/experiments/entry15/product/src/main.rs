#![no_std]
#![no_main]
entry_api::entry!(run);
trait BufferBoard { fn into_buffer(self) -> [u8; 2048]; }
struct FakeBoard;
impl BufferBoard for FakeBoard { fn into_buffer(self) -> [u8; 2048] { [0x5a; 2048] } }
async fn app<B: BufferBoard>(board: B) {
    let mut buffer = board.into_buffer();
    core::hint::black_box(&mut buffer);
    core::future::pending::<()>().await;
    core::hint::black_box(&mut buffer);
}
async fn run(_: entry_api::embassy_executor::Spawner) {
    #[cfg(not(feature = "async-main"))]
    {
        core::hint::black_box(&ENTRY15_MAIN_LAYOUT);
        core::hint::black_box(&ENTRY15_PRODUCT_LAYOUT);
        core::hint::black_box(&ENTRY15_CHIP);
    }
    app(FakeBoard).await;
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
