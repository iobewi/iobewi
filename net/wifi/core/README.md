---
layer: portable-contract
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-wifi-core

## Summary

Portable Wi-Fi contracts: station transport port and the provisioning capability

## Responsibilities

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-contract` boundary.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture

Path: `net/wifi/core`. Layer: **portable-contract**.

## Public API

Exported Rust items are the code-level API authority. Package features and dependency declarations remain canonical in `Cargo.toml`.

## Invariants

- `INV-001`

## Validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.
