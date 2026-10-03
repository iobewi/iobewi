# Agent Context — iobewi-crypto-mbedtls

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-crypto-mbedtls`
- Path: `crypto/mbedtls`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

MbedTLS implementation of the TLS-material crypto contract: PEM/X.509 validation, identity generation, hooks

## Owns

- Own the capability, policy or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Does not own

- Does not redefine portable policy that belongs in platform-independent contracts.
- Does not own unrelated product/application composition.

## Architecture position

This crate lives at `crypto/mbedtls` and is classified as **platform-adapter**. It implements platform-specific behaviour behind IOBEWI boundaries.

Local path dependencies declared by Cargo include:
- `../core`
- `../rng`
- `../../time/core`

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
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
