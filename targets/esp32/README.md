# targets/esp32

Cargo workspace for the ESP32 (S3 / C3) implementations. It holds **build and
toolchain configuration only** (`Cargo.toml`, `Cargo.lock`): the member crates stay in
the subsystem that owns their semantics. Build from this directory with the `esp`
toolchain, e.g.:

```sh
cargo +esp check -Z build-std=core,alloc --target xtensa-esp32s3-none-elf -p iobewi-esp-wifi --features esp32s3
```

Members (each declares `workspace = "<path>/targets/esp32"`): `arch/esp32/{reset,runtime}`,
`crypto/mbedtls`, `drivers/led/ws2812/esp32`, `drivers/{console,device,flash,hw_random,net/{tcp,tls,wifi},watchdog}/esp32`,
`drivers/flash/partitions-esp32`, `fs/nvs/{esp32,config-esp32}`, `firmware/esp32`.

The ESP bootloader is **not** a member: `bootloader/esp` keeps an autonomous workspace and lockfile.
The former provisional `iobewi-esp/` subtree was removed in S13 (its history stays in earlier commits).

## ESP support policy

- Supported esp-hal line: **1.2.x** (manifests carry `~1.2`, i.e. `>=1.2.0 <1.3.0`).
- `Cargo.lock` (here and in `bootloader/esp`) is the reproducible, validated baseline. CI
  `esp (locked)` builds with `--locked`.
- CI `esp (latest)` removes the lockfile in its checkout only and resolves the newest
  dependencies the manifests allow (newest 1.2.x). It runs on push/PR and daily.
- Moving to esp-hal 1.3 is an explicit migration (manifests + lockfiles), never automatic.

Notes on the 1.2 baseline: `esp-nvs` 0.5 (latest) caps `esp-storage` at `<0.10`, so
`iobewi-esp-nvs` does not enable its chip features and supplies the ROM CRC itself
(`esp-rom-sys`); the Wi-Fi adapter targets `esp-radio =1.0.0-beta.1`; the status LED uses
`esp-hal-smartled` 0.18 (the pulse-width compensation needed with `esp-hal-smartled2` is gone).
