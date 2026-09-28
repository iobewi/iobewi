# iobewi-esp ESP bootloader

Feature-driven Rust `no_std` second-stage bootloader owned by `iobewi-esp`.

```text
ESP ROM
  -> iobewi-esp-bootloader
       -> iobewi-esp-platform / iobewi-esp-boot   (hardware)
       -> fibewi::boot                  (firmware lifecycle semantics)
  -> ota_0 / ota_1
  -> application
```

The executable owns the ESP execution boundary: HAL runtime, ROM flash access,
watchdog handoff, MMU/cache mapping, RAM loading, linker profile and final jump.

FiBeWI remains the policy/semantic dependency. It decides which image may boot
and which EWBT transitions are required, but it does not own an ESP executable.

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
duplicating FiBeWI boot semantics.
