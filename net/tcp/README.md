---
layer: portable-service
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-net-tcp

## Summary

TCP listener and accepted async stream over an `embassy-net` stack, implementing the portable net/io contracts. No hardware dependency: it runs on any platform that provides an `embassy-net` stack. Formerly `iobewi-esp-tcp` under `drivers/net/tcp/esp32`; moved and renamed because nothing in it is ESP-specific.

## Responsibilities

Accept TCP connections using caller-owned buffers and provide reads, writes and graceful FIN/flush closure.

## Non-responsibilities

HTTP routes, TLS certificates/handshake, Wi-Fi initialization and selecting which product ports are exposed.

## Architecture

Portable net/io implementation over `embassy-net`, underneath the TLS listener (`drivers/net/tls/esp32`); the HTTP service is layered above by the product. Path: `net/tcp`.

## Public API

`TcpListener::new(stack, port, rx, tx)`, `accept_connection`, ConnectionListener implementation and `TcpStream` implementing embedded async I/O and Close.

## Invariants

- `INV-001`: no platform dependency; it needs only an `embassy-net` stack.

## Validation

Checked by `cargo check --workspace` on the host and, on the target, through the TLS driver (`cargo +esp check -p iobewi-esp-tls --features esp32s3,embassy-net`). There are no host tests: a listener needs a running stack. BG-ESP-S3 validates TCP/TLS listener composition on hardware.

## Known limitations

Accepted connections borrow listener buffers, so they cannot outlive that borrow or run concurrently from the same buffers. Accept errors are reduced to (). This primitive can listen on a supplied port; securing management and keeping plaintext port 80 closed is the product composition policy, not a transport-enforced rule.

## Related components

`net/io`, `drivers/net/tls/esp32`, `net/http/server`.
