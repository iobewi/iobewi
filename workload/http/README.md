---
layer: portable-service
status: implemented
invariants:
  - INV-001
  - INV-002
  - INV-003
  - INV-009
  - INV-011
  - INV-017
  - INV-018
  - INV-019
gates:
  - BG-WORKLOAD-OTA
---

# iobewi-workload-ota-http

## Summary

HTTP routes of the Workload OTA (prepare / streaming write / activate / status) over iobewi-http-server and iobewi-workload-ota; no platform type

## Responsibilities

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-service` boundary.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture

Path: `workload/http`. Layer: **portable-service**.

Local IOBEWI dependencies declared by Cargo:
- `../update`
- `../../firmware/model`
- `../../firmware/update`
- `../../net/http/server`
- `../update`
- `../../net/io`

## Public API

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

## Validation

- `BG-WORKLOAD-OTA`

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.
