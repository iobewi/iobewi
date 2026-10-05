# Agent Context — Entry15 product experiment

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `entry15-product-proof`
- Path: `tools/experiments/entry15/product`
- Layer: `experimental-product`
- Status: `experimental`

## Role

Downstream product fixture with a renamed facade dependency and no direct HAL/RTOS dependency.

## Owns

Exercise generic source composition retaining a 2048-byte buffer across await and export compile-time layouts.

## Does not own

No production Board, serial/console retirement, USB handover, storage, Wi-Fi,
OTA or hardware qualification. Not a framework API to integrate into products.

## Architecture position

Isolated experimental workspace; no changes to production workspace membership.
See [experiment instructions](../README.md). All crates are unpublished.

## Public contracts

The entry is entry_api::entry!(run). A minimal local BufferBoard is solely a future-size probe, not the approved Board contract.

## Invariants

INV-001: HAL dependencies stay in the experimental facade, not portable app logic.
No independently loaded Workload ABI is involved.

## Modification context

See the canonical README and implementation.

## Required validation

Build only -p entry15-product-proof on Xtensa in release mode using the commands
in the parent README, then inspect the linked ELF. docs_tool.py check verifies
this README and generated AGENTS. Compilation does not satisfy BG-ESP-S3 hardware.

## Known limitations

The entry task never completes in this fixture. No allocator/resource budget is
initialized or validated. The panic handler belongs to the fixture. Future sizes
are fixture-specific, not StreamBeWI memory requirements. Chip support is S3 only.
The negative async-main mode is expected to fail and is not a production feature.

## Related components

[Experiment instructions](../README.md), issue #15 and proposed ADR-0015 in PR #16.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
