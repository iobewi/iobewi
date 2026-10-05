#![no_std]
pub use esp_hal;
pub use esp_metadata_generated;
pub use esp_rtos;
pub use embassy_executor;
pub use esp_bootloader_esp_idf;

pub const fn future_layout<F: embassy_executor::_export::TaskFn<(embassy_executor::Spawner,)>>(_: F) -> [u32; 2] {
    [core::mem::size_of::<F::Fut>() as u32, core::mem::align_of::<F::Fut>() as u32]
}

#[cfg(not(feature = "async-main"))]
#[macro_export]
macro_rules! entry {
    ($run:path) => {
        use $crate::esp_hal;
        $crate::esp_bootloader_esp_idf::esp_app_desc!();
        #[$crate::embassy_executor::task(embassy_executor = $crate::embassy_executor)]
        async fn __entry15_task(spawner: $crate::embassy_executor::Spawner) {
            let p = $crate::esp_hal::init(Default::default());
            let t = $crate::esp_hal::timer::timg::TimerGroup::new(p.TIMG0);
            $crate::esp_rtos::start(t.timer0, p.FROM_CPU_INTR0);
            $run(spawner).await;
        }
        #[used]
        #[unsafe(no_mangle)]
        pub static ENTRY15_CHIP: [u8; 8] = *$crate::esp_metadata_generated::chip_pretty!().as_bytes().first_chunk::<8>().unwrap();
        #[used]
        #[unsafe(no_mangle)]
        pub static ENTRY15_MAIN_LAYOUT: [u32; 2] = $crate::future_layout(____entry15_task_task);
        #[used]
        #[unsafe(no_mangle)]
        pub static ENTRY15_PRODUCT_LAYOUT: [u32; 2] = $crate::future_layout($run);
        #[esp_hal::main]
        fn main() -> ! {
            let mut executor = $crate::esp_rtos::embassy::Executor::new();
            // SAFETY: main never returns and this executor stays on its stack forever.
            let executor = unsafe { &mut *(&mut executor as *mut $crate::esp_rtos::embassy::Executor) };
            executor.run(|spawner| { spawner.spawn(__entry15_task(spawner).unwrap()); })
        }
    };
}

#[cfg(feature = "async-main")]
include!("async_entry.rs");
