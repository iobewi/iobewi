# Agent Context — iobewi-net-io

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-net-io`
- Path: `net/io`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Low-level connection contracts: Close, Connection, ConnectionListener, outbound Connector (no HTTP, TLS, Wi-Fi or platform types)

## Owns

- Own the contract/service/platform mechanism described above.
- Preserve the `portable-contract` boundary.

## Does not own

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.

## Architecture position

Path: `net/io`. Layer: **portable-contract**.

## Public contracts

- `Close` adds asynchronous clean shutdown to an `ErrorType`.
- `Connection` bundles `Read + Write + Close`; matching streams implement it automatically.
- `ConnectionListener::accept` borrows the listener and yields an inbound connection or `()`; retry delays and platform logging belong to the listener.
- `Connector::connect(host, port, rx, tx)` opens an outbound connection. Its implementation owns name resolution and connection establishment. The caller supplies transport buffers borrowed for the connection lifetime; connection failures expose an error implementing `Display`.
- `Connector::local_address` optionally reports a local address and defaults to `None`.

These contracts do not imply TLS or authentication. Such guarantees belong to refinement contracts in the owning subsystem. Package features and dependency declarations remain canonical in `Cargo.toml`.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- Focused crate/workspace tests; no additional hardware baseline gate is declared.

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
