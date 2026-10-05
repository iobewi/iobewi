---
layer: platform-adapter
status: implemented
invariants:
  - INV-001
  - INV-004
  - INV-005
gates:
  - BG-STORAGE
  - BG-ESP-S3
---

# iobewi-esp-config-space

## Summary

ESP NVS adapter for IOBEWI ConfigSpace

## Responsibilities

- Implement the portable ConfigSpace backend contract using ESP NVS and the shared ESP flash path.
- Use portable NVS record and reservation calculations from `iobewi-nvs-core`.
- Keep that responsibility inside the `platform-adapter` layer.

## Non-responsibilities

- Does not create or own an independent physical flash instance.
- Does not define product configuration policy.

## Architecture

Path: `fs/nvs/config-esp32`. Layer: **platform-adapter**.

Local IOBEWI path dependencies declared by Cargo:
- `../../config`
- `../core`
- `../../../drivers/flash/esp32`
- `../esp32`

## Public API

`NvsConfigBackend` implements `ConfigBackend`. Construction receives an existing `&'static SharedFlash` and `NvsPartition`; flash access is serialized through that shared owner. `is_healthy` and `self_check` expose backend health checks. `NvsPartition` and NVS capacity constants are re-exported for composition.

Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- `INV-001`
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
