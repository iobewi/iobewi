---
layer: platform-adapter
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-watchdog

## Summary

ESP timer-group watchdog hardware primitive

## Responsibilities

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Non-responsibilities

- Does not redefine portable policy owned by platform-independent crates.
- Does not own unrelated product/application composition.

## Architecture

Path: `drivers/watchdog/esp32`. Layer: **platform-adapter**.

## Public API

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Validation

- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.
