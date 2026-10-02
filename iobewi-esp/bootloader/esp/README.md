# iobewi-esp ESP bootloader

Feature-driven Rust `no_std` second-stage bootloader owned by `iobewi-esp`.

```text
ESP ROM
  -> iobewi-esp-bootloader
       -> iobewi-esp-platform / iobewi-esp-boot   (hardware)
       -> iobewi-firmware-boot / iobewi-firmware-image  (EWBT; ESP image validation)
  -> ota_0 / ota_1
  -> application
```

The executable owns the ESP execution boundary: HAL runtime, ROM flash access,
watchdog handoff, MMU/cache mapping, RAM loading, linker profile and final jump.

`iobewi-firmware-boot` supplies the EWBT policy and `iobewi-firmware-image` the
ESP image validation to the bootloader. The executable applies those decisions using the ESP hardware layer.

## Targets

Supported bootloader targets:

| Feature | Rust target | Linker profile |
| --- | --- | --- |
| `esp32c3` | `riscv32imc-unknown-none-elf` | `linker/iobewi-esp-boot-esp32c3.x` |
| `esp32s3` | `xtensa-esp32s3-none-elf` | `linker/iobewi-esp-boot-esp32s3.x` |

## Build

```sh
cd bootloader/esp
cargo build --release --locked \
  --features esp32c3 \
  --target riscv32imc-unknown-none-elf
```

No target is enabled by default. Additional SoCs add an `iobewi-esp-platform`
profile, an `iobewi-esp-boot` hardware backend and a linker profile without
duplicating IOBEWI OTA boot semantics.

## ESP32-S3 memory map (and why it is where it is)

The ROM loads the image at flash offset 0 into SRAM and jumps to its entry
point; this bootloader owns only the window below, and must stay out of the
ROM's data and of the data-cache window.

| Region (data-bus address) | Use |
|---|---|
| `0x3fcdb000..0x3fce4000` | code: IRAM alias `0x403cb000..0x403d4000` (vectors `0x403cb000..0x403cb400`, then `.rwtext`/`.text`) |
| `0x3fce4000..0x3fcec000` | **DRAM**: `.data`, `.rodata`, `.bss`, stack (initial SP `0x3fcec000`, grows down) |
| `0x3fced710..0x3fcf0000` | ROM data (`rom_spiflash_legacy_data` `0x3fceffe4`, ...) -- never touch |
| `0x3fcf8000..0x3fd00000` | data cache (32 KiB, set by esp-hal's `esp32_init` before any Rust code; 64 KiB would start at `0x3fcf0000`) |

The first S3 layout had DRAM at `0x3fcf4000..0x3fcfc000`, so the stack was
inside the data-cache window: `esp32_init` reconfigured the cache under the
running stack and the next return double-faulted (`_DoubleExceptionVector`)
before a single line of output. ESP32-C3 has no such overlap (its DRAM ends at
`0x3fcdc000`, below the ROM data at `0x3fcdc710`). The linker script now
asserts the rules above, so a bad layout fails the link instead of the boot.
