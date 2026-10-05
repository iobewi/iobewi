---
layer: platform-adapter
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-tcp

## Summary

ESP TCP transport: an embassy-net listener and accepted socket as net/io connections

## Responsibilities

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Non-responsibilities

- Does not redefine portable policy owned by platform-independent crates.
- Does not own unrelated product/application composition.

## Architecture

Path: `drivers/net/tcp/esp32`. Layer: **platform-adapter**.

Local IOBEWI path dependencies declared by Cargo:
- `../../../../net/io`

## Public API

- `EspTcpListener::new` binds an Embassy network stack, listening port and caller-owned receive/transmit buffers.
- `accept_connection` accepts an inbound TCP socket.
- `EspTcpListener` implements `ConnectionListener`, returning `EspTcpStream` connections.
- `EspTcpStream` implements asynchronous read/write and clean close.

Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Validation

- `BG-ESP-S3`

## Known limitations

Only inbound TCP connections are implemented. This crate currently provides no outbound `Connector`, DNS resolution or product reconnection policy.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.
