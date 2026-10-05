# Agent Context — Entry15 facade experiment

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `entry15-facade`
- Path: `tools/experiments/entry15/facade`
- Layer: `experimental-platform-adapter`
- Status: `experimental`

## Role

Experimental ESP32-S3 entry facade; not a production Board implementation.

## Owns

Emit a downstream descriptor and concrete Embassy task, initialize HAL/RTOS and launch the caller future. Preserve a failing async-main variant to reproduce macro hygiene limits.

## Does not own

No production Board, serial/console retirement, USB handover, storage, Wi-Fi,
OTA or hardware qualification. Not a framework API to integrate into products.

## Architecture position

Isolated experimental workspace; no changes to production workspace membership.
See [experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md). All crates are unpublished.

## Public contracts

Exports entry!, dependency reexports and future_layout(). Default uses blocking esp_hal::main plus an explicit task with an overridden executor path; async-main reproduces the upstream absolute-path failure.

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

[Experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md), issue #15 and proposed ADR-0015 in PR #16.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
