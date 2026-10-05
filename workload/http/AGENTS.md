# Agent Context — iobewi-workload-ota-http

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-workload-ota-http`
- Path: `workload/http`
- Layer: `portable-service`
- Status: `implemented`

## Role

HTTP routes of the Workload OTA (prepare / streaming write / activate / status) over iobewi-http-server and iobewi-workload-ota; no platform type

## Owns

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-service` boundary.

## Does not own

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture position

Path: `workload/http`. Layer: **portable-service**.

Local IOBEWI dependencies declared by Cargo:
- `../update`
- `../../firmware/model`
- `../../firmware/update`
- `../../net/http/server`
- `../update`
- `../../net/io`

## Public contracts

Exported Rust items are the code-level API authority. Package features and dependency declarations remain canonical in `Cargo.toml`.

## Invariants

- `INV-001`
- `INV-002`
- `INV-003`
- `INV-009`
- `INV-011`
- `INV-017`
- `INV-018`
- `INV-019`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-WORKLOAD-OTA`

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
