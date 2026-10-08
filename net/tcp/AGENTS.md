# Agent Context — iobewi-net-tcp

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-net-tcp`
- Path: `net/tcp`
- Layer: `portable-service`
- Status: `implemented`

## Role

TCP listener and accepted async stream over an `embassy-net` stack, implementing the portable net/io contracts. No hardware dependency: it runs on any platform that provides an `embassy-net` stack. Formerly `iobewi-esp-tcp` under `drivers/net/tcp/esp32`; moved and renamed because nothing in it is ESP-specific.

## Owns

Accept TCP connections using caller-owned buffers and provide reads, writes and graceful FIN/flush closure.

## Does not own

HTTP routes, TLS certificates/handshake, Wi-Fi initialization and selecting which product ports are exposed.

## Architecture position

Portable net/io implementation over `embassy-net`, underneath the TLS listener (`drivers/net/tls/esp32`); the HTTP service is layered above by the product. Path: `net/tcp`.

## Public contracts

`TcpListener::new(stack, port, rx, tx)`, `accept_connection`, ConnectionListener implementation and `TcpStream` implementing embedded async I/O and Close.

## Invariants

- `INV-001`: no platform dependency; it needs only an `embassy-net` stack.

## Modification context

See the canonical README and implementation.

## Required validation

Checked by `cargo check --workspace` on the host and, on the target, through the TLS driver (`cargo +esp check -p iobewi-esp-tls --features esp32s3,embassy-net`). There are no host tests: a listener needs a running stack. BG-ESP-S3 validates TCP/TLS listener composition on hardware.

## Known limitations

Accepted connections borrow listener buffers, so they cannot outlive that borrow or run concurrently from the same buffers. Accept errors are reduced to (). This primitive can listen on a supplied port; securing management and keeping plaintext port 80 closed is the product composition policy, not a transport-enforced rule.

## Related components

`net/io`, `drivers/net/tls/esp32`, `net/http/server`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
