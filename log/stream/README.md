---
layer: portable-service
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-log-stream

## Summary

Best-effort outbound WebSocket delivery of captured logs over a supplied secure client transport.

## Responsibilities

Connect and upgrade with a bearer token, mask client frames using shared entropy, attach node/workload/time metadata, drain captured messages and reconnect with capped jittered backoff.

## Non-responsibilities

TLS setup, certificate persistence, local console output, reliable delivery and application configuration schema.

## Architecture

Portable service above log/core, net/http/client and net/tls/core. The product implements both StreamConfig and LogMetadata.

## Public API

`StreamConfig` supplies `ctrl_url`, `token` and `path`. `run<C, T, E>(&C, &T, &E) -> !` requires `StreamConfig + LogMetadata`, `SecureClientTransport` and `EntropySource`.

## Invariants

- `INV-001`

## Validation

`cargo test -p iobewi-log-stream` covers URL/backoff and entropy. For product changes involving delivery, run BG-ESP-S3 with TLS/token rotation and reconnection evidence.

## Known limitations

Uses alloc and Embassy time. Clears the ring after failed/ended sessions or absent URL/token; no offline replay or delivery acknowledgement. Backoff starts at 5 s, caps at 60 s, uses ±30% jitter and resets after a 30 s stable session. Handshake and incomplete incoming frame timeouts are 10 s. JSON level is `raw`. URL parsing is basic host/port parsing, not a general IPv6 URL parser; transport security comes from the injected secure transport.

## Related components

`log/core`, `net/http/client`, `net/tls/core`, `crypto/rng`.
