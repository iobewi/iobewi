# Agent Context — iobewi-ota-http

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-ota-http`
- Path: `net/http/ota`
- Layer: `portable-service`
- Status: `implemented`

## Role

HTTP routes of the OTA workflow (prepare / streaming write / activate) over iobewi-http-server and iobewi-ota

## Owns

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-service` layer.

## Does not own

- Does not access a platform HAL directly.
- Does not own hardware-specific implementation details.

## Architecture position

Path: `net/http/ota`. Layer: **portable-service**.

Local IOBEWI path dependencies declared by Cargo:
- `../../../firmware/update`
- `../server`

## Public contracts

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- `INV-001`
- `INV-003`
- `INV-009`
- `INV-017`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-AGENT-OTA`

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
