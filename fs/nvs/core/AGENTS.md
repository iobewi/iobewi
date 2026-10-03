# Agent Context — iobewi-nvs-core

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-nvs-core`
- Path: `fs/nvs/core`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Chip-independent logic of the NVS-backed ConfigSpace persistence: record framing, generations, key rules and the NVS entry-accounting / capacity formulas

## Owns

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-contract` layer.

## Does not own

- Does not access a platform HAL directly.
- Does not own hardware-specific implementation details.

## Architecture position

Path: `fs/nvs/core`. Layer: **portable-contract**.

## Public contracts

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-STORAGE`

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: root `AGENTS.md`, `ARCHITECTURE.md`, `INVARIANTS.md` and referenced contracts/ADRs/gates.
