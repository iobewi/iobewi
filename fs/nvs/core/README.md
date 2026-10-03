---
layer: portable-contract
status: implemented
invariants:
  - INV-001
gates:
  - BG-STORAGE
---

# iobewi-nvs-core

## Summary

Chip-independent logic of the NVS-backed ConfigSpace persistence: record framing, generations, key rules and the NVS entry-accounting / capacity formulas

## Responsibilities

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-contract` layer.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own hardware-specific implementation details.

## Architecture

Path: `fs/nvs/core`. Layer: **portable-contract**.

## Public API

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- `INV-001`

## Validation

- `BG-STORAGE`

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.
