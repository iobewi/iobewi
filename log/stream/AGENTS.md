# Agent Context — iobewi-log-stream

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-log-stream`
- Path: `log/stream`
- Layer: `portable-service`
- Status: `implemented`

## Role

Best-effort outbound WebSocket delivery of captured logs over a supplied secure client transport.

## Owns

Connect and upgrade with a bearer token, mask client frames using shared entropy, attach node/workload/time metadata, drain captured messages and reconnect with capped jittered backoff.

## Does not own

TLS setup, certificate persistence, local console output, reliable delivery and application configuration schema.

## Architecture position

Portable service above log/core, net/http/client and net/tls/core. The product implements both StreamConfig and LogMetadata.

## Public contracts

`StreamConfig` supplies `ctrl_url`, `token` and `path`. `run<C, T, E>(&C, &T, &E) -> !` requires `StreamConfig + LogMetadata`, `SecureClientTransport` and `EntropySource`.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

`cargo test -p iobewi-log-stream` covers URL/backoff, entropy and serialization of every captured level/target with JSON escaping. For product changes involving delivery, run BG-ESP-S3 with TLS/token rotation and reconnection evidence.

## Known limitations

Uses alloc and Embassy time. Clears the ring after failed/ended sessions or absent URL/token; no offline replay or delivery acknowledgement. Backoff starts at 5 s, caps at 60 s, uses ±30% jitter and resets after a 30 s stable session. Handshake and incomplete incoming frame timeouts are 10 s. JSON preserves the original lowercase level (`error`, `warn`, `info`, `debug`, `trace`) and adds `target`; `ts`, `node`, `workload` and `msg` retain their meanings. Consumers must accept the additive `target` field and actual levels instead of `raw`. Capture policy and network authorization remain separate. URL parsing is basic host/port parsing, not a general IPv6 URL parser; transport security comes from the injected secure transport.

## Related components

`log/core`, `net/http/client`, `net/tls/core`, `crypto/rng`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
