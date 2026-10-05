---
layer: portable-service
status: implemented
invariants:
  - INV-001
  - INV-020
gates:
  - BG-STORAGE
  - BG-ESP-S3
---

# iobewi-esp-partitions

## Summary

Policy-free ESP-IDF partition table and raw partition helpers

## Responsibilities

- Own the capability, policy or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-service` layer.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own product/application composition beyond this crate's stated contract.

## Architecture

This crate lives at `drivers/flash/partitions-esp32` and is classified as **portable-service**. It must preserve the portable-to-platform dependency direction.

## Public API

The Rust items exported by this crate are the code-level API authority. Consumers should depend on the semantic capability described here and avoid coupling to private implementation details. Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- [INV-001](../../../INVARIANTS.md)
- [INV-020](../../../INVARIANTS.md)

## Validation

- `BG-STORAGE`
- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded here beyond the repository current-state and open-debt documents. Add limitations here when they affect callers or modification safety.

## Related components

- [Repository architecture](../../../ARCHITECTURE.md)
- [Repository invariants](../../../INVARIANTS.md)
- `Cargo.toml` for package features and dependency facts.
