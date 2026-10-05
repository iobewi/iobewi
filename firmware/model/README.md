---
layer: portable-contract
status: implemented
invariants:
  - INV-001
  - INV-002
  - INV-003
  - INV-009
  - INV-011
  - INV-021
gates: []
---

# iobewi-update-model

## Summary

Dual-OTA model: Agent and Workload update targets, runtime-API compatibility, A/B slot sets and the two separate activation policies (no storage, no platform, no transport)

## Responsibilities

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-contract` layer.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own hardware-specific implementation details.

## Architecture

Path: `firmware/model`. Layer: **portable-contract**.

## Public API

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- `INV-001`
- `INV-002`
- `INV-003`
- `INV-009`
- `INV-011`
- `INV-021`

## Validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.
