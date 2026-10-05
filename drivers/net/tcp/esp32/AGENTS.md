# Agent Context — iobewi-esp-tcp

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-tcp`
- Path: `drivers/net/tcp/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

ESP TCP transport: an embassy-net listener and accepted socket as net/io connections

## Owns

- Own the capability, state model, service or platform mechanism described in the summary.
- Keep that responsibility inside the `platform-adapter` layer.

## Does not own

- Does not redefine portable policy owned by platform-independent crates.
- Does not own unrelated product/application composition.

## Architecture position

Path: `drivers/net/tcp/esp32`. Layer: **platform-adapter**.

Local IOBEWI path dependencies declared by Cargo:
- `../../../../net/io`

## Public contracts

- `EspTcpListener::new` binds an Embassy network stack, listening port and caller-owned receive/transmit buffers.
- `accept_connection` accepts an inbound TCP socket.
- `EspTcpListener` implements `ConnectionListener`, returning `EspTcpStream` connections.
- `EspTcpStream` implements asynchronous read/write and clean close.

Package features and dependency declarations are canonical in `Cargo.toml`.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-ESP-S3`

## Known limitations

Only inbound TCP connections are implemented. This crate currently provides no outbound `Connector`, DNS resolution or product reconnection policy.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
