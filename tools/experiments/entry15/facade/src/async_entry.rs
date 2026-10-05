#[macro_export]
macro_rules! entry {
    ($run:path) => {
        use $crate::{esp_hal, esp_rtos, embassy_executor};
        $crate::esp_bootloader_esp_idf::esp_app_desc!();
        #[esp_hal::main]
        async fn main(spawner: $crate::embassy_executor::Spawner) {
            let p = $crate::esp_hal::init(Default::default());
            let t = $crate::esp_hal::timer::timg::TimerGroup::new(p.TIMG0);
            $crate::esp_rtos::start(t.timer0, p.FROM_CPU_INTR0);
            $run(spawner).await;
        }
    };
}
