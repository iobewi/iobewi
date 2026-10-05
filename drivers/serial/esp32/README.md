---
layer: platform-driver
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-serial

## Summary

ESP32-S3 UART and USB-Serial-JTAG halves implementing embedded-io-async 0.7.

## Responsibilities

Own typed RX/TX adapters and a finite, consuming EspSerialBank.

## Non-responsibilities

No Improv parser, console sink, USB mode policy or transport response routing.

## Architecture

The platform creates UART0 and optionally JTAG after boot-mode selection; products receive portable Read/Write halves through Board.

## Public API

EspSerialBank::new(uart, optional_jtag) supplies UART first, then JTAG. take_next permanently returns None after exhaustion. RX/TX errors are exposed as SerialError.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Validation

Cross-compile the Board fixture in provisioning and mass-storage modes.

## Known limitations

Only S3 is enabled initially. Async protocol operations can wait on a stalled host; products must bound protocol waits. This is not a console backend.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.
