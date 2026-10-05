#![no_std]
pub use embassy_executor;
pub use esp_bootloader_esp_idf;
pub use esp_hal;
pub use esp_metadata_generated;
pub use esp_rtos;

// Experiment-only contracts. These are not the proposed production Board API.
pub trait SharedFlashAccess {
    fn read_marker(&mut self) -> u8;
}
pub struct BoardParts<F> {
    pub flash: F,
    pub buffer: [u8; 2048],
}
pub trait Board: Sized {
    type Flash: SharedFlashAccess;
    fn into_parts(self) -> BoardParts<Self::Flash>;
}
// A non-Clone owning token simulates a shared-flash capability; no physical flash
// is created, and this does not prove NVS discovery or multicore parking.
pub struct MockSharedFlash {
    marker: u8,
}
impl SharedFlashAccess for MockSharedFlash {
    fn read_marker(&mut self) -> u8 {
        self.marker
    }
}
pub struct MockBoard {
    flash: MockSharedFlash,
}
impl MockBoard {
    #[doc(hidden)]
    pub fn new_for_experiment() -> Self {
        Self {
            flash: MockSharedFlash { marker: 0x5a },
        }
    }
}
impl Board for MockBoard {
    type Flash = MockSharedFlash;
    fn into_parts(self) -> BoardParts<Self::Flash> {
        BoardParts {
            flash: self.flash,
            buffer: [0x5a; 2048],
        }
    }
}
pub const HEAP_BYTES: usize = 96 * 1024;
#[doc(hidden)]
pub fn initialize_experiment_heap() {
    // Called exactly once by the single entry. No production resource validation.
    esp_alloc::heap_allocator!(size: HEAP_BYTES);
}
pub const fn future_layout<F, Args>(_: F) -> [u32; 2]
where
    F: embassy_executor::_export::TaskFn<Args>,
{
    [
        core::mem::size_of::<F::Fut>() as u32,
        core::mem::align_of::<F::Fut>() as u32,
    ]
}

#[cfg(not(feature = "async-main"))]
#[macro_export]
macro_rules! entry {
    ($run:path) => {
        // Reserved module, emitted once per binary. Imports stay out of the
        // product's root namespace; mandatory entry/descriptor symbols remain.
        #[doc(hidden)]
        mod __iobewi_entry15 {
            use super::*;
            use $crate::esp_hal;
            $crate::esp_bootloader_esp_idf::esp_app_desc!();
            #[$crate::embassy_executor::task(embassy_executor = $crate::embassy_executor)]
            async fn __entry15_task(board: $crate::MockBoard) {
                core::hint::black_box(&CHIP);
                core::hint::black_box(&MAIN_LAYOUT);
                core::hint::black_box(&PRODUCT_LAYOUT);
                core::hint::black_box(&HEAP_LAYOUT);
                $run(board).await;
            }
            #[used]
            #[unsafe(export_name = "__iobewi_entry15_experiment_chip")]
            static CHIP: [u8; 8] = *$crate::esp_metadata_generated::chip_pretty!()
                .as_bytes()
                .first_chunk::<8>()
                .unwrap();
            #[used]
            #[unsafe(export_name = "__iobewi_entry15_experiment_main_layout")]
            static MAIN_LAYOUT: [u32; 2] =
                $crate::future_layout::<_, ($crate::MockBoard,)>(____entry15_task_task);
            #[used]
            #[unsafe(export_name = "__iobewi_entry15_experiment_product_layout")]
            static PRODUCT_LAYOUT: [u32; 2] =
                $crate::future_layout::<_, ($crate::MockBoard,)>($run);
            #[used]
            #[unsafe(export_name = "__iobewi_entry15_experiment_heap_bytes")]
            static HEAP_LAYOUT: u32 = $crate::HEAP_BYTES as u32;
            #[esp_hal::main]
            fn main() -> ! {
                let config = $crate::esp_hal::Config::default()
                    .with_cpu_clock($crate::esp_hal::clock::CpuClock::max());
                let p = $crate::esp_hal::init(config);
                $crate::initialize_experiment_heap();
                let t = $crate::esp_hal::timer::timg::TimerGroup::new(p.TIMG0);
                $crate::esp_rtos::start(t.timer0, p.FROM_CPU_INTR0);
                let board = $crate::MockBoard::new_for_experiment();
                let mut executor = $crate::esp_rtos::embassy::Executor::new();
                // SAFETY: main never returns; executor stays on its stack forever.
                let executor =
                    unsafe { &mut *(&mut executor as *mut $crate::esp_rtos::embassy::Executor) };
                executor.run(|spawner| {
                    spawner.spawn(__entry15_task(board).unwrap());
                })
            }
        }
    };
}

#[cfg(feature = "async-main")]
include!("async_entry.rs");
