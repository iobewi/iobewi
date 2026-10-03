---
layer: portable-service
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-log-stream

## Summary

Portable outbound WebSocket streaming of captured logs

## Responsibilities

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-service` layer.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own hardware-specific implementation details.

## Architecture

Path: `log/stream`. Layer: **portable-service**.

Local IOBEWI path dependencies declared by Cargo:
- `../../crypto/rng`
- `../core`
- `../../net/http/client`
- `../../net/io`
- `../../net/tls/core`

## Public API

The exported Rust items are the code-level API authority. Package features and dependency declarations are canonical in `Cargo.toml`; callers should depend on the semantic capability documented here, not private implementation details.

## Invariants

- `INV-001`

## Validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.
