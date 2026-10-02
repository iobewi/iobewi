#![no_std]

//! Pure ESP platform descriptors.
//!
//! This crate contains hardware facts only: chip identifiers and memory
//! geometry consumed by boot/image logic. It has no HAL, flash, NVS, OTA,
//! rollback or application dependencies and therefore remains host-testable.

use core::ops::Range;

/// Memory geometry required to validate and load an ESP application image.
#[derive(Clone, Debug)]
pub struct MemoryMap {
    pub chip_id: u16,
    /// Flash-mapped through the MMU.
    pub drom: Range<u32>,
    pub irom: Range<u32>,
    /// Internal SRAM instruction-bus and data-bus aliases.
    pub iram: Range<u32>,
    pub dram: Range<u32>,
    pub rtc: Range<u32>,
    /// `iram - sram_alias_offset == dram`.
    pub sram_alias_offset: u32,
    /// Memory occupied by the second-stage bootloader/ROM while loading.
    pub boot_window: Range<u32>,
    /// MMU page size used for flash-mapped segments.
    pub mmu_page: u32,
}

impl MemoryMap {
    pub fn is_flash_mapped(&self, addr: u32) -> bool {
        self.drom.contains(&addr) || self.irom.contains(&addr)
    }
}

pub mod chips {
    pub mod esp32c3 {
        use crate::MemoryMap;

        /// ESP32-C3 boot memory geometry for the current IOBEWI OTA second-stage
        /// loader RAM window.
        ///
        /// The concrete bootloader linker script must stay consistent with
        /// `boot_window`; the ESP implementation owns both hardware descriptions as the boot
        /// platform is extracted.
        ///
        /// Sources: `drom`/`irom`/`iram`/`dram`/`rtc` and `chip_id` are
        /// esptool's own per-chip `MEMORY_MAP`/chip-id tables (the same
        /// authority a real flash tool uses to decide what is legal);
        /// `sram_alias_offset` is `SOC_I_D_OFFSET` from esp-idf's
        /// `soc/esp32c3/include/soc/soc.h` (`SOC_DIRAM_IRAM_LOW -
        /// SOC_DIRAM_DRAM_LOW`).
        pub const BOOT_MEMORY_MAP: MemoryMap = MemoryMap {
            chip_id: 0x0005,
            drom: 0x3C00_0000..0x3C80_0000,
            irom: 0x4200_0000..0x4280_0000,
            iram: 0x4037_C000..0x403E_0000,
            dram: 0x3FC8_0000..0x3FCE_0000,
            rtc: 0x5000_0000..0x5000_2000,
            sram_alias_offset: 0x0070_0000,
            boot_window: 0x3FCC_B000..0x3FCE_0000,
            mmu_page: 0x1_0000,
        };
    }

    pub mod esp32s3 {
        use crate::MemoryMap;

        /// ESP32-S3 boot memory geometry for the IOBEWI OTA second-stage loader
        /// RAM window.
        ///
        /// Sources, same methodology as ESP32-C3's map: `drom`/`irom`/`iram`/
        /// `dram` and `rtc` are esptool's `esp32s3.py` `MEMORY_MAP` table
        /// (the `rtc` range is the entry esptool itself labels
        /// `RTC_IRAM`/`RTC_DRAM`, at 0x600fe000 -- ESP32-S3 additionally has
        /// a *second*, larger RTC bank at 0x50000000 esptool calls
        /// `RTC_DATA`/esp-hal calls `rtc_slow`, which ESP32-C3 has no
        /// equivalent of and which this single-range field does not need for
        /// boot validation). `chip_id` is espflash's own `Chip::Esp32s3`
        /// image chip-id. `sram_alias_offset` is `SOC_I_D_OFFSET` from
        /// esp-idf's `soc/esp32s3/include/soc/soc.h`.
        ///
        /// `boot_window` must stay consistent with the concrete bootloader
        /// linker script (ESP bootloader `boot-esp32s3.x` at
        /// the time of writing). Deliberately generous: a first hardware
        /// test with a straight C3-sized reservation (0x9000/0x8000)
        /// panicked with a corrupted panic location, consistent with
        /// Xtensa's windowed-register spill overflowing a stack sized for
        /// RISC-V's flat register file. S3 has ample spare SRAM for this
        /// corner, so there is no cost to sizing it generously.
        pub const BOOT_MEMORY_MAP: MemoryMap = MemoryMap {
            chip_id: 0x0009,
            drom: 0x3C00_0000..0x3D00_0000,
            irom: 0x4200_0000..0x4280_0000,
            iram: 0x4037_0000..0x403E_0000,
            dram: 0x3FC8_8000..0x3FD0_0000,
            rtc: 0x600F_E000..0x6010_0000,
            sram_alias_offset: 0x006F_0000,
            boot_window: 0x3FCC_8000..0x3FD0_0000,
            mmu_page: 0x1_0000,
        };
    }
}
