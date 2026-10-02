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
