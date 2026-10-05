# Agent Context — iobewi-esp-platform

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-platform`
- Path: `arch/esp32/platform`
- Layer: `platform-architecture`
- Status: `implemented`

## Role

Pure `no_std`, host-testable ESP32-C3/S3 memory descriptors for boot/image logic.

## Owns

- Describe chip image identifiers, address ranges, SRAM aliases and the bootloader reservation.
- Classify a single address as flash-mapped without accessing hardware.

## Does not own

No GPIO/pinout, board wiring, peripheral ownership, HAL/RTOS startup, flash access,
partition discovery or OTA/rollback policy. A memory map is not a complete Board profile.

## Architecture position

Hardware facts at `arch/esp32/platform`, with no dependencies. Image validation
and boot adapters consume these values; target linker scripts own actual placement.

## Public contracts

- `MemoryMap` is a cloneable descriptor with public fields: `chip_id`, half-open
  `drom`/`irom` flash ranges, `iram`/`dram` SRAM aliases, `rtc`,
  `sram_alias_offset`, `boot_window` and `mmu_page` (bytes).
- `MemoryMap::is_flash_mapped(addr)` tests membership in DROM or IROM only.
  It does not validate a segment length, image, partition or available capacity.
- `chips::esp32c3::BOOT_MEMORY_MAP` and `chips::esp32s3::BOOT_MEMORY_MAP`
  are available together without chip features. Image chip IDs are `0x0005`
  and `0x0009`; both use 64 KiB MMU pages. Exact ranges are in `src/lib.rs`.

## Invariants

Repository-wide invariants apply; this crate declares no additional invariant.

## Modification context

### Lifecycle

Descriptors require no initialization or allocator and acquire no resource.
Keep `boot_window` consistent with the matching C3/S3 bootloader linker script
under `arch/esp32/{c3,s3}/linker/`; do not treat that reserved region as free RAM.
No operation returns an error; callers own complete image/bounds validation.

## Required validation

Host: `cargo check -p iobewi-esp-platform -p iobewi-esp-boot --features iobewi-esp-boot/esp32s3`.
Changes to geometry require image/boot validation and `BG-ESP-S3` with the concrete
linker layout; a documentation check alone is not hardware qualification.

## Known limitations

Only C3 and S3 maps exist. S3's `rtc` describes `0x600fe000..0x60100000`,
not its additional RTC slow-memory bank at `0x50000000`. These boot-oriented
ranges do not enumerate all usable memory or describe external PSRAM.

## Related components

- [ESP boot binding](../boot/README.md)
- [Firmware image contracts](../../../firmware/image/README.md)
- [Product integration](../../../docs/product-integration.md)
- `src/lib.rs` and `Cargo.toml` are authoritative for values and dependencies.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
