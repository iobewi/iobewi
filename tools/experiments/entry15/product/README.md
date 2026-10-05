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

Exercise run<B: Board>(board) through entry!, retaining a 2048-byte buffer, a simulated flash token and a 128-byte Box across await. Exercise old root-name collisions.

## Non-responsibilities

No production Board, boot-mode USB selection, physical storage, Wi-Fi,
OTA or hardware qualification. Not a framework API to integrate into products.

## Architecture

Isolated experimental workspace; no changes to production workspace membership.
See [experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md). All crates are unpublished.

## Public API

The entry is entry_api::entry!(crate::run). The generic product consumes a Board with an associated SharedFlashAccess bound. These reduced experiment-only contracts are future-size probes, not the approved complete Board API. A facade HAL reexport import exists solely as a namespace collision probe outside the generic run.

## Invariants

INV-001: HAL dependencies stay in the experimental facade, not portable app logic.
No independently loaded Workload ABI is involved.

## Validation

Build only -p entry15-product-proof on Xtensa in release mode using the commands
in the parent README, then inspect the linked ELF. docs_tool.py check verifies
this README and generated AGENTS. Compilation does not satisfy BG-ESP-S3 hardware.

## Known limitations

The entry task never completes in this fixture. A 96-KiB heap is initialized,
but no production resource budget or runtime heap/stack high-water mark is validated. The panic handler belongs to the fixture. Future sizes
are fixture-specific, not StreamBeWI memory requirements. Chip support is S3 only.
The negative async-main mode is expected to fail and is not a production feature.

## Related components

[Experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md), issue #15 and proposed ADR-0015 in PR #16.
