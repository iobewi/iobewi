# Agent Context — iobewi-log

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-log`
- Path: `log/core`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Local log capture: bounded ring and global logger, no network dependency

## Owns

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-contract` layer.

## Does not own

- Does not access a platform HAL directly.
- Does not own hardware-specific implementation details.

## Architecture position

Path: `log/core`. Layer: **portable-contract**.

## Public contracts

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
