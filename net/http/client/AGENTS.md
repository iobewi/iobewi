# Agent Context — iobewi-http-client

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-http-client`
- Path: `net/http/client`
- Layer: `portable-service`
- Status: `implemented`

## Role

Outbound HTTP/1.1 and optional WebSocket primitives over connected async I/O.

## Owns

Serialize authenticated JSON POST requests, consume one HTTP response body and report safe connection reuse; implement the optional outbound WebSocket upgrade/frame helpers.

## Does not own

DNS, dialing, TLS, certificates, server routes, request cadence and retry policy.

## Architecture position

Portable protocol layer over embedded_io_async Read/Write. TLS service and log streaming inject established connections.

## Public contracts

`post_json(session, host, path, bearer, json)` writes and flushes without closing. `drain_response(session, scratch)` returns `(status, reusable)`. Feature `websocket` exports upgrade, frame processing and text-send helpers.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

`cargo test -p iobewi-http-client --features websocket` covers request bytes, framing, truncation and WebSocket behaviour.

## Known limitations

Uses alloc. Requests reject CR/LF in host/path/bearer and require an absolute path. Response headers must fit scratch and at most 16 parsed headers. Content-Length and chunked bodies are discarded incrementally. Requests/responses are lock-step: pipelined bytes can be discarded. Without body framing, reuse is false. Caller owns deadlines and connection closure; this is not a general browser HTTP client.

## Related components

`log/stream`, `net/tls/core`, `net/tls/service`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
