---
layer: experimental-product
status: experimental
invariants:
  - INV-001
gates:
  - BG-ESP-S3
---

# Entry15 product experiment

## Summary

Downstream product fixture with a renamed facade dependency and no direct HAL/RTOS dependency.

## Responsibilities

Exercise generic source composition retaining a 2048-byte buffer across await and export compile-time layouts.

## Non-responsibilities

No production Board, serial/console retirement, USB handover, storage, Wi-Fi,
OTA or hardware qualification. Not a framework API to integrate into products.

## Architecture

Isolated experimental workspace; no changes to production workspace membership.
See [experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md). All crates are unpublished.

## Public API

The entry is entry_api::entry!(run). A minimal local BufferBoard is solely a future-size probe, not the approved Board contract.

## Invariants

INV-001: HAL dependencies stay in the experimental facade, not portable app logic.
No independently loaded Workload ABI is involved.

## Validation

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
