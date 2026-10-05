---
layer: validation-tool
status: implemented
invariants: []
gates: []
---

# board15-product-proof

## Summary

Downstream compile/link fixture using the real S3 Board startup.

## Responsibilities

Exercise generic run<B: Board>, a product resource request, NVS configuration access and both exclusive USB constructors.

## Non-responsibilities

No StreamBeWI service join, actual flag persistence, hardware execution or representative product memory measurement.

## Architecture

The firmware owns a separate workspace, renames the facade dependency to entry_api and uses the framework build helper. Its product source contains no HAL types.

## Public API

BOARD_RESOURCES requests 3 sockets, 96 KiB heap and a 16 KiB minimum linker stack. Default mode compiles provisioning; mass-storage compiles the OTG branch.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Validation

bash tools/experiments/board15/run.sh locked; the inspector checks downstream image name/version/chip and reports future layouts.

## Known limitations

The binary is a fixture and is not flashed by this gate. NVS discovery errors and warm-reset PHY state require hardware. Firmware compilation uses the ESP toolchain, not host cargo test.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.
