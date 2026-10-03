# Agent Context — iobewi-net-tls-core

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-net-tls-core`
- Path: `net/tls/core`
- Layer: `portable-contract`
- Status: `implemented`

## Role

TLS network contracts without identity/ConfigSpace policy: the secure-outbound-connector guarantee

## Owns

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-contract` boundary.

## Does not own

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture position

Path: `net/tls/core`. Layer: **portable-contract**.

Local IOBEWI dependencies declared by Cargo:
- `../../io`

## Public contracts

Exported Rust items are the code-level API authority. Package features and dependency declarations remain canonical in `Cargo.toml`.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: root `AGENTS.md`, `ARCHITECTURE.md`, `INVARIANTS.md`, and referenced contracts/ADRs/gates.
