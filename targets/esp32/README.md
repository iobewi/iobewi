# targets/esp32

Cargo workspace for the ESP32 (S3 / C3) implementations. It holds **build and
toolchain configuration only** (`Cargo.toml`, `Cargo.lock`): the member crates stay in
the subsystem that owns their semantics. Build from this directory with the `esp`
toolchain, e.g.:

```sh
cargo +esp check -Z build-std=core,alloc --target xtensa-esp32s3-none-elf -p iobewi-esp-wifi --features esp32s3
```

Members (each declares `workspace = "<path>/targets/esp32"`): `arch/esp32/{reset,runtime}`,
`crypto/mbedtls`, `drivers/{console,device,flash,hw_random,indicator,net/{tcp,tls,wifi},watchdog}/esp32`,
`drivers/flash/partitions-esp32`, `fs/nvs/{esp32,config-esp32}`, `firmware/esp32`.

The ESP bootloader is **not** a member: `bootloader/esp` keeps an autonomous workspace and lockfile.
The former provisional `iobewi-esp/` subtree was removed in S13 (its history stays in earlier commits).
