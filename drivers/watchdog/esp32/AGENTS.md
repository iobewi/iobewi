# Agent Context — iobewi-esp-watchdog

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-watchdog`
- Path: `drivers/watchdog/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

ESP timer-group watchdog hardware primitive

## Owns

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Does not own

- Does not redefine portable policy owned by platform-independent crates.
- Does not own unrelated product/application composition.

## Architecture position

Path: `drivers/watchdog/esp32`. Layer: **platform-adapter**.

## Public contracts

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: root `AGENTS.md`, `ARCHITECTURE.md`, `INVARIANTS.md` and referenced contracts/ADRs/gates.
