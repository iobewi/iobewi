# Agent Context — iobewi-esp-tcp

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-tcp`
- Path: `drivers/net/tcp/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

Embassy-net TCP listener and accepted async stream implementing portable net/io contracts.

## Owns

Accept TCP connections using caller-owned buffers and provide reads, writes and graceful FIN/flush closure.

## Does not own

HTTP routes, TLS certificates/handshake, Wi-Fi initialization and selecting which product ports are exposed.

## Architecture position

Platform net/io implementation underneath the ESP TLS listener; HTTP service is layered above by the product.

## Public contracts

`EspTcpListener::new(stack, port, rx, tx)`, `accept_connection`, ConnectionListener implementation and `EspTcpStream` implementing embedded async I/O and Close.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Modification context

See the canonical README and implementation.

## Required validation

Build targets/esp32 for the selected chip; BG-ESP-S3 validates TCP/TLS listener composition on hardware.

## Known limitations

Accepted connections borrow listener buffers, so they cannot outlive that borrow or run concurrently from the same buffers. Accept errors are reduced to (). This primitive can listen on a supplied port; securing management and keeping plaintext port 80 closed is the product composition policy, not a transport-enforced rule.

## Related components

`net/io`, `drivers/net/tls/esp32`, `net/http/server`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
