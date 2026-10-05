---
layer: portable-contract
status: implemented
invariants:
  - INV-001
  - INV-002
  - INV-003
  - INV-006
  - INV-007
  - INV-008
  - INV-010
  - INV-011
gates: []
---

# iobewi-workload-abi

## Summary

Binary contract between the Agent and a native Workload: WorkloadContextV1, the control block and the service tables (repr(C), fixed-size types, extern C only; no Rust ABI crosses it)

## Responsibilities

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-contract` boundary.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture

Path: `workload/abi`. Layer: **portable-contract**.

## Public API

Exported Rust items are the code-level API authority. Package features and dependency declarations remain canonical in `Cargo.toml`.

## Invariants

- `INV-001`
- `INV-002`
- `INV-003`
- `INV-006`
- `INV-007`
- `INV-008`
- `INV-010`
- `INV-011`

## Validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.
