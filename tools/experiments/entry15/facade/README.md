---
layer: experimental-platform-adapter
status: experimental
invariants:
  - INV-001
gates:
  - BG-ESP-S3
---

# Entry15 facade experiment

## Summary

Experimental ESP32-S3 entry facade; not a production Board implementation.

## Responsibilities

Emit a downstream descriptor and concrete Embassy task, initialize HAL/RTOS and a 96-KiB allocator, then construct an owning MockBoard and launch the generic caller future. Preserve a failing async-main variant to reproduce macro hygiene limits.

## Non-responsibilities

No production Board, boot-mode USB selection, physical storage, Wi-Fi,
OTA or hardware qualification. Not a framework API to integrate into products.

## Architecture

Isolated experimental workspace; no changes to production workspace membership.
See [experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md). All crates are unpublished.

## Public API

Exports entry!, dependency reexports, generic future_layout(), and experiment-only Board/SharedFlashAccess capabilities. Macro internals live in a reserved hidden module with reserved ELF probe names. Default uses blocking esp_hal::main plus an explicit task with an overridden executor path; async-main reproduces the upstream absolute-path failure.

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
