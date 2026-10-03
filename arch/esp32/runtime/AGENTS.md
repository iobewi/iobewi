# Agent Context — iobewi-esp-runtime

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-runtime`
- Path: `arch/esp32/runtime`
- Layer: `platform-architecture`
- Status: `implemented`

## Role

ESP/Xtensa/RISC-V stack high-water-mark measurement for the portable IOBEWI runtime diagnostics capability

## Owns

- Own the capability, policy or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-architecture` layer.

## Does not own

- Does not redefine portable policy that belongs in platform-independent contracts.
- Does not own unrelated product/application composition.

## Architecture position

This crate lives at `arch/esp32/runtime` and is classified as **platform-architecture**. It implements platform-specific behaviour behind IOBEWI boundaries.

Local path dependencies declared by Cargo include:
- `../../../drivers/diagnostics/core`

## Public contracts

The Rust items exported by this crate are the code-level API authority. Consumers should depend on the semantic capability described here and avoid coupling to private implementation details. Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- No additional crate-specific repository invariant is declared; repository-wide rules still apply.

## Modification context

See the canonical README and implementation.

## Required validation

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
