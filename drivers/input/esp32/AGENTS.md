# Agent Context — iobewi-esp-input

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-input`
- Path: `drivers/input/esp32`
- Layer: `platform-driver`
- Status: `implemented`

## Role

S3 reference-profile BOOT button binding.

## Owns

Construct GPIO0 input with pull-up for an active-low button.

## Does not own

No debounce, recovery policy, GPIO inventory or generic pin selection.

## Architecture position

The platform owns the GPIO token; the product receives embedded-hal-async Wait through Board.

## Public contracts

boot_button(GPIO0) returns a HAL Input implementing the portable async Wait contract.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Modification context

See the canonical README and implementation.

## Required validation

Cross-compile the Board fixture; verify button polarity on BG-ESP-S3 hardware.

## Known limitations

Only the first GPIO0 wiring profile is implemented; a package pin being available does not imply a physical button.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
