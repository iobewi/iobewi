# Agent Context — iobewi-tls-http

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-tls-http`
- Path: `net/http/tls`
- Layer: `portable-service`
- Status: `implemented`

## Role

HTTP provisioning routes of the TLS identity/trust service (cert / CA) over iobewi-http-server and iobewi-tls-service

## Owns

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-service` boundary.

## Does not own

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture position

Path: `net/http/tls`. Layer: **portable-service**.

Local IOBEWI dependencies declared by Cargo:
- `../server`
- `../../tls/service`

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
