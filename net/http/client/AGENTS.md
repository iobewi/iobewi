# Agent Context — iobewi-http-client

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-http-client`
- Path: `net/http/client`
- Layer: `portable-service`
- Status: `implemented`

## Role

Portable JSON POST, response draining and optional WebSocket client over a connected asynchronous stream

## Owns

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `portable-service` layer.

## Does not own

- Does not access a platform HAL directly.
- Does not own hardware-specific implementation details.

## Architecture position

Path: `net/http/client`. Layer: **portable-service**.

## Public contracts

- `post_json` sends an authenticated JSON POST and leaves the connection open.
- `drain_response` discards one response body incrementally and returns its HTTP status and whether the connection can be reused. It handles Content-Length and chunked bodies; the caller supplies scratch space large enough for the complete headers.
- The optional `websocket` module provides upgrade, frame processing and text sending over the connected stream.

Connection establishment, TLS and certificate policy belong to transport adapters. Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

There is no GET API exposing response body bytes to a consumer, and no HTTP live-streaming API. `drain_response` discards bodies rather than delivering them. Requests and responses must be strictly sequential: pipelined bytes read beyond a response may be dropped. Headers must fit the supplied scratch buffer.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
