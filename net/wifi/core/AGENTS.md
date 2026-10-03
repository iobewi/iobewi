# Agent Context — iobewi-wifi-core

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-wifi-core`
- Path: `net/wifi/core`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Portable Wi-Fi contracts: station transport port and the provisioning capability

## Owns

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-contract` boundary.

## Does not own

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture position

Path: `net/wifi/core`. Layer: **portable-contract**.

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
