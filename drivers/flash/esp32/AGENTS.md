# Agent Context — iobewi-esp-flash

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-flash`
- Path: `drivers/flash/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

Process-wide ESP physical flash ownership and serialized raw access

## Owns

- Own the capability, policy or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Does not own

- Does not redefine portable policy that belongs in platform-independent contracts.
- Does not own unrelated product/application composition.

## Architecture position

This crate lives at `drivers/flash/esp32` and is classified as **platform-adapter**. It implements platform-specific behaviour behind IOBEWI boundaries.

## Public contracts

The Rust items exported by this crate are the code-level API authority. Consumers should depend on the semantic capability described here and avoid coupling to private implementation details. Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- [INV-004](../../../INVARIANTS.md)
- [INV-005](../../../INVARIANTS.md)

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-STORAGE`
- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded here beyond the repository current-state and open-debt documents. Add limitations here when they affect callers or modification safety.

## Related components

- [Repository architecture](../../../ARCHITECTURE.md)
- [Repository invariants](../../../INVARIANTS.md)
- `Cargo.toml` for package features and dependency facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`, `INVARIANTS.md`, and referenced contracts/ADRs/gates.
