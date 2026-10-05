---
layer: platform-adapter
status: implemented
invariants:
  - INV-002
  - INV-003
  - INV-004
  - INV-005
  - INV-009
  - INV-020
  - INV-021
gates:
  - BG-STORAGE
  - BG-WORKLOAD-OTA
  - BG-NATIVE-RUNTIME
  - BG-ESP-S3
---

# iobewi-esp-workload

## Summary

ESP storage for the Workload OTA: partition lookup by name, capability, SharedFlash-serialized OTM2 metadata and Workload A/B slot access

## Responsibilities

- Own the contract/service/platform mechanism described above.
- Preserve the `platform-adapter` boundary.

## Non-responsibilities

- Does not redefine portable Workload policy or control-plane semantics.
- Does not own unrelated product composition.

## Architecture

Path: `workload/esp32`. Layer: **platform-adapter**.

Local IOBEWI dependencies declared by Cargo:
- `../update`
- `../../firmware/model`
- `../../firmware/update`
- `../../drivers/flash/esp32`
- `../../drivers/flash/partitions-esp32`
- `../native`
- `../abi`
- `../image`

## Public API

Exported Rust items are the code-level API authority. Package features and dependency declarations remain canonical in `Cargo.toml`.

## Invariants

- `INV-002`
- `INV-003`
- `INV-004`
- `INV-005`
- `INV-009`
- `INV-020`
- `INV-021`

## Validation

- `BG-STORAGE`
- `BG-WORKLOAD-OTA`
- `BG-NATIVE-RUNTIME`
- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.
