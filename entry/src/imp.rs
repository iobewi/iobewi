pub use iobewi_esp_runtime::{
    EspRuntimeDiagnostics,
    platform::{EspBoard, Startup},
};
pub use {
    embassy_executor, embassy_net, esp_alloc, esp_bootloader_esp_idf, esp_hal,
    esp_metadata_generated, esp_rtos, static_cell,
};
pub const fn future_layout<F, Args>(_: F) -> [u32; 2]
where
    F: embassy_executor::_export::TaskFn<Args>,
{
    [
        core::mem::size_of::<F::Fut>() as u32,
        core::mem::align_of::<F::Fut>() as u32,
    ]
}
/// Launch a generic product with its portable BOARD_RESOURCES declaration.
/// Initially selects the S3 native-USB wiring profile; hardware qualification
/// and StreamBeWI memory measurements are still required for final acceptance.
#[macro_export]
macro_rules! entry {
    ($product:ident :: run) => { $crate::entry!($product::run, resources = $product::BOARD_RESOURCES); };
    ($run:path, resources = $resources:path) => {
        #[doc(hidden)]
        mod __iobewi_entry {
            use super::*;
            use $crate::esp_hal;
            const REQUEST: $crate::ResourceRequest = $resources;
            type Board = $crate::EspBoard<{ REQUEST.sockets }>;
            $crate::esp_bootloader_esp_idf::esp_app_desc!();
            #[$crate::embassy_executor::task(embassy_executor = $crate::embassy_executor)]
            async fn __iobewi_task(spawner: $crate::embassy_executor::Spawner, startup: $crate::Startup<{ REQUEST.sockets }>) {
                core::hint::black_box(&MAIN_LAYOUT);
                core::hint::black_box(&PRODUCT_LAYOUT);
                core::hint::black_box(&CHIP);
                let board = match startup.finish(spawner).await { Ok(board) => board, Err(error) => error.halt() };
                $run(board).await;
            }
            #[used]
            #[unsafe(export_name = "__iobewi_entry_main_layout")]
            static MAIN_LAYOUT: [u32;2] = $crate::future_layout::<_, ($crate::embassy_executor::Spawner, $crate::Startup<{ REQUEST.sockets }>)>(____iobewi_task_task);
            #[used]
            #[unsafe(export_name = "__iobewi_entry_product_layout")]
            static PRODUCT_LAYOUT: [u32;2] = $crate::future_layout::<_, (Board,)>($run);
            #[used]
            #[unsafe(export_name = "__iobewi_entry_chip")]
            static CHIP: [u8;8] = *$crate::esp_metadata_generated::chip_pretty!().as_bytes().first_chunk::<8>().unwrap();
            #[esp_hal::main]
            fn main() -> ! {
                let _diagnostics = $crate::EspRuntimeDiagnostics::initialize();
                let p = $crate::esp_hal::init($crate::esp_hal::Config::default().with_cpu_clock($crate::esp_hal::clock::CpuClock::max()));
                static RESOURCES: $crate::static_cell::StaticCell<$crate::embassy_net::StackResources<{ REQUEST.sockets }>> = $crate::static_cell::StaticCell::new();
                static USB_OUT: $crate::static_cell::StaticCell<[u8;1024]> = $crate::static_cell::StaticCell::new();
                let startup = $crate::Startup::prepare(p, REQUEST, RESOURCES.init($crate::embassy_net::StackResources::new()), USB_OUT.init([0;1024]), || {
                    $crate::esp_alloc::heap_allocator!(size: REQUEST.heap_bytes);
                });
                let mut executor = $crate::esp_rtos::embassy::Executor::new();
                // SAFETY: the non-returning main keeps this executor on its stack.
                let executor = unsafe { &mut *(&mut executor as *mut $crate::esp_rtos::embassy::Executor) };
                executor.run(|spawner| { spawner.spawn(__iobewi_task(spawner, startup).unwrap()); })
            }
            #[panic_handler]
            fn panic(_: &core::panic::PanicInfo) -> ! {
                // No println/backtrace backend, and no peripheral acquisition.
                loop { core::hint::spin_loop(); }
            }
        }
    };
}
