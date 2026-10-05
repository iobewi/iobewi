---
layer: experimental-build-tool
status: experimental
invariants:
  - INV-001
gates:
  - BG-ESP-S3
---

# Entry15 build-helper experiment

## Summary

Host build helper proving downstream linker arguments must originate in the product build script.

## Responsibilities

Emit -Tlinkall.x from the downstream build.rs.

## Non-responsibilities

No production Board, boot-mode USB selection, physical storage, Wi-Fi,
OTA or hardware qualification. Not a framework API to integrate into products.

## Architecture

Isolated experimental workspace; no changes to production workspace membership.
See [experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md). All crates are unpublished.

## Public API

emit() prints the linker argument; this crate is a host build dependency, not embedded code.

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
