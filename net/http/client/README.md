---
layer: portable-service
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-http-client

## Summary

Portable JSON POST, response draining and optional WebSocket client over a connected asynchronous stream

## Responsibilities

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-service` layer.

## Non-responsibilities

- Does not access a platform HAL directly.
- Does not own hardware-specific implementation details.

## Architecture

Path: `net/http/client`. Layer: **portable-service**.

## Public API

- `post_json` sends an authenticated JSON POST and leaves the connection open.
- `drain_response` discards one response body incrementally and returns its HTTP status and whether the connection can be reused. It handles Content-Length and chunked bodies; the caller supplies scratch space large enough for the complete headers.
- The optional `websocket` module provides upgrade, frame processing and text sending over the connected stream.

Connection establishment, TLS and certificate policy belong to transport adapters. Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- `INV-001`

## Validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

There is no GET API exposing response body bytes to a consumer, and no HTTP live-streaming API. `drain_response` discards bodies rather than delivering them. Requests and responses must be strictly sequential: pipelined bytes read beyond a response may be dropped. Headers must fit the supplied scratch buffer.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.
