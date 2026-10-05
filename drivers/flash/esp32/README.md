---
layer: platform-adapter
status: implemented
invariants:
  - INV-004
  - INV-005
gates:
  - BG-STORAGE
  - BG-ESP-S3
---

# iobewi-esp-flash

## Summary

Process-wide ESP physical flash ownership and serialized raw access

## Responsibilities

- Own the capability, policy or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Non-responsibilities

- Does not redefine portable policy that belongs in platform-independent contracts.
- Does not own unrelated product/application composition.

## Architecture

This crate lives at `drivers/flash/esp32` and is classified as **platform-adapter**. It implements platform-specific behaviour behind IOBEWI boundaries.

## Public API

- `init(FLASH)` creates the process-wide flash owner and returns `&'static SharedFlash`.
- `SharedFlash` is an asynchronous Embassy mutex around `EspFlash`.
- `EspFlash` implements the synchronous `ReadNorFlash`, `NorFlash` and `MultiwriteNorFlash` traits. `storage()` exposes the underlying ESP driver while the caller holds exclusive access.

Package features and dependency declarations are canonical in `Cargo.toml`.

## Lifecycle

The composition root must call `init` exactly once per firmware image. All flash consumers share the returned mutex. It is not reentrant: release the guard before invoking another subsystem that locks the same flash, including ConfigSpace.

With the S3 feature, initialization enables `multicore_auto_park` so flash operations safely pause and resume a Workload executing on the second core.

## Invariants

- [INV-004](../../../INVARIANTS.md)
- [INV-005](../../../INVARIANTS.md)

## Validation

- `BG-STORAGE`
- `BG-ESP-S3`

## Known limitations

Repeated initialization is unsupported. Nested acquisition of the shared mutex cannot complete; callers must preserve the lock ordering described above.

## Related components

- [Repository architecture](../../../ARCHITECTURE.md)
- [Repository invariants](../../../INVARIANTS.md)
- `Cargo.toml` for package features and dependency facts.
