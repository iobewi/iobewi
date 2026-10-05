---
layer: platform-adapter
status: implemented
invariants:
  - INV-004
  - INV-005
  - INV-009
  - INV-020
gates:
  - BG-AGENT-OTA
  - BG-STORAGE
  - BG-ESP-S3
---

# iobewi-esp-ota

## Summary

ESP partition and NOR-flash adapter for IOBEWI OTA

## Responsibilities

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Non-responsibilities

- Does not redefine portable policy owned by platform-independent crates.
- Does not own unrelated product/application composition.

## Architecture

Path: `firmware/esp32`. Layer: **platform-adapter**.

Local IOBEWI path dependencies declared by Cargo:
- `../update`
- `../boot`
- `../slots`
- `../../drivers/flash/partitions-esp32`
- `../../drivers/flash/esp32`
- `../../fs/nvs/config-esp32`
- `../../drivers/watchdog/esp32`
- `../../arch/esp32/reset`

## Public API

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- `INV-004`
- `INV-005`
- `INV-009`
- `INV-020`

## Validation

- `BG-AGENT-OTA`
- `BG-STORAGE`
- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.
