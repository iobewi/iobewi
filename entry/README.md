---
layer: platform-architecture
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-entry

## Summary

Framework-owned entry candidate for a generic Board product on ESP32-S3.

## Responsibilities

Initialize HAL at maximum CPU clock, heap, RTOS, a concrete executor task and platform Board. Embed downstream image name/version and measurable future layouts.

## Non-responsibilities

No product flag interpretation, provisioning/persistence policy, Improv routing, service joins or USB class identity.

## Architecture

The facade reexports dependencies used by its macro. A reserved module contains concrete Embassy task wrappers; the product run remains generic over Board. Startup delegates existing flash, NVS, Wi-Fi and device adapters.

## Public API

entry!(product::run) uses product::BOARD_RESOURCES: ResourceRequest. The explicit form is entry!(path::run, resources = path::REQUEST). Select esp32s3 and call iobewi_entry_build::emit() in build.rs. The async product is run<B: Board>(board: B).

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Validation

Run bash tools/experiments/board15/run.sh locked; inspect descriptor, chip and future symbols. Run the ESP CI matrix and required hardware gates.

## Known limitations

This is a compilation-qualified candidate, not final entry acceptance. Real StreamBeWI future/heap/stack and BG-ESP-S3/BG-USB-MSC remain pending. Module __iobewi_entry and exported __iobewi_entry_* symbols are reserved. The macro supplies a silent panic handler; no println/backtrace backend is linked. The only startup-fatal output is best-effort UART0. Other chips and profiles are rejected.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.
