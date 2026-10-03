---
layer: platform-architecture
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-reset

## Summary

ESP32 SoC reset primitives: digital-core software reset and an RTC-watchdog system reset (no executor, no network, no firmware policy)

## Responsibilities

- Own the capability, policy or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-architecture` layer.

## Non-responsibilities

- Does not redefine portable policy that belongs in platform-independent contracts.
- Does not own unrelated product/application composition.

## Architecture

This crate lives at `arch/esp32/reset` and is classified as **platform-architecture**. It implements platform-specific behaviour behind IOBEWI boundaries.

## Public API

The Rust items exported by this crate are the code-level API authority. Consumers should depend on the semantic capability described here and avoid coupling to private implementation details. Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- No additional crate-specific repository invariant is declared; repository-wide rules still apply.

## Validation

- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded here beyond the repository current-state and open-debt documents. Add limitations here when they affect callers or modification safety.

## Related components

- [Repository architecture](../../../ARCHITECTURE.md)
- [Repository invariants](../../../INVARIANTS.md)
- `Cargo.toml` for package features and dependency facts.
