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

# iobewi-esp-nvs

## Summary

ESP NVS hardware bridge over iobewi-esp-flash

## Responsibilities

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Non-responsibilities

- Does not redefine portable policy owned by platform-independent crates.
- Does not own unrelated product/application composition.

## Architecture

Path: `fs/nvs/esp32`. Layer: **platform-adapter**.

Local IOBEWI path dependencies declared by Cargo:
- `../../../drivers/flash/esp32`

## Public API

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- `INV-004`
- `INV-005`

## Validation

- `BG-STORAGE`
- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.
