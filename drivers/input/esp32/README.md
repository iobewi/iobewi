---
layer: platform-driver
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-input

## Summary

S3 reference-profile BOOT button binding.

## Responsibilities

Construct GPIO0 input with pull-up for an active-low button.

## Non-responsibilities

No debounce, recovery policy, GPIO inventory or generic pin selection.

## Architecture

The platform owns the GPIO token; the product receives embedded-hal-async Wait through Board.

## Public API

boot_button(GPIO0) returns a HAL Input implementing the portable async Wait contract.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Validation

Cross-compile the Board fixture; verify button polarity on BG-ESP-S3 hardware.

## Known limitations

Only the first GPIO0 wiring profile is implemented; a package pin being available does not imply a physical button.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.
