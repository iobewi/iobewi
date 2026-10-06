# Agent Context — board15-product-proof

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `board15-product-proof`
- Path: `tools/experiments/board15/product`
- Layer: `validation-tool`
- Status: `implemented`

## Role

Downstream compile/link fixture using the real S3 Board startup.

## Owns

Exercise generic run<B: Board>, a product resource request, NVS configuration access and both exclusive USB constructors.

## Does not own

No StreamBeWI service join, actual flag persistence, hardware execution or representative product memory measurement.

## Architecture position

The firmware owns a separate workspace, renames the facade dependency to entry_api and uses the framework build helper. Its product source contains no HAL types.

## Public contracts

BOARD_RESOURCES requests 3 sockets, 96 KiB heap and a 16 KiB minimum linker stack. Default mode compiles provisioning; mass-storage compiles the OTG branch.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Modification context

See the canonical README and implementation.

## Required validation

bash tools/experiments/board15/run.sh locked; the inspector checks downstream image name/version/chip and reports future layouts.

## Known limitations

The binary is a fixture and is not flashed by this gate. NVS discovery errors and warm-reset PHY state require hardware. Firmware compilation uses the ESP toolchain, not host cargo test.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
