# Agent Context — iobewi-esp-bootloader

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-bootloader`
- Path: `bootloader/esp`
- Layer: `platform-executable`
- Status: `implemented`

## Role

Feature-driven Rust `no_std` ESP second-stage bootloader executor using IOBEWI Agent OTA lifecycle semantics.

## Owns

- Own the ESP boot execution boundary: HAL/ROM flash access, watchdog handoff, MMU/cache mapping, RAM loading, linker profile and final jump.
- Apply portable `iobewi-firmware-boot` decisions and `iobewi-firmware-image` validation on ESP hardware.

## Does not own

- Does not own Workload slot activation; the bootloader never chooses Workload A/B.
- Does not own HTTP, provisioning, Workload supervision or application policy.

## Architecture position

This is a platform executable with its own autonomous workspace and lockfile. ESP ROM loads this bootloader; it consumes ESP platform/boot primitives plus portable firmware boot/image logic, then transfers control to the selected Agent application slot.

## Public contracts

The executable interface is its boot behaviour and feature-selected target build. `esp32c3` targets `riscv32imc-unknown-none-elf`; `esp32s3` targets `xtensa-esp32s3-none-elf`. There is no library API.

## Invariants

- `INV-003`
- `INV-009`
- `INV-017`
- `INV-021`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-AGENT-OTA`
- `BG-ESP-S3`

## Known limitations

The linker/memory layout is SoC-specific. On ESP32-S3, DRAM/stack must remain below ROM-data and cache windows; linker assertions are part of the safety boundary.

## Related components

- `arch/esp32/boot`
- `arch/esp32/platform`
- `firmware/boot`
- `firmware/image`

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: root `AGENTS.md`, `ARCHITECTURE.md`, `INVARIANTS.md`, and referenced contracts/ADRs/gates.
