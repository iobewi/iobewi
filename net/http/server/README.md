---
layer: portable-service
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-http-server

## Summary

Portable HTTP server: router, serve loop and the net/io connection adapter (picoserve-based)

## Responsibilities

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-service` boundary.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture

Path: `net/http/server`. Layer: **portable-service**.

Local IOBEWI dependencies declared by Cargo:
- `../../io`
- `../../tls/core`

## Public API

Exported Rust items are the code-level API authority. Package features and dependency declarations remain canonical in `Cargo.toml`.

## Invariants

- `INV-001`

## Validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.
