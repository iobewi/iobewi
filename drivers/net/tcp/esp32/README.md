---
layer: platform-adapter
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-tcp

## Summary

Embassy-net TCP listener and accepted async stream implementing portable net/io contracts.

## Responsibilities

Accept TCP connections using caller-owned buffers and provide reads, writes and graceful FIN/flush closure.

## Non-responsibilities

HTTP routes, TLS certificates/handshake, Wi-Fi initialization and selecting which product ports are exposed.

## Architecture

Platform net/io implementation underneath the ESP TLS listener; HTTP service is layered above by the product.

## Public API

`EspTcpListener::new(stack, port, rx, tx)`, `accept_connection`, ConnectionListener implementation and `EspTcpStream` implementing embedded async I/O and Close.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Validation

Build targets/esp32 for the selected chip; BG-ESP-S3 validates TCP/TLS listener composition on hardware.

## Known limitations

Accepted connections borrow listener buffers, so they cannot outlive that borrow or run concurrently from the same buffers. Accept errors are reduced to (). This primitive can listen on a supplied port; securing management and keeping plaintext port 80 closed is the product composition policy, not a transport-enforced rule.

## Related components

`net/io`, `drivers/net/tls/esp32`, `net/http/server`.
