# Agent Context — Entry15 build-helper experiment

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `entry15-build-helper`
- Path: `tools/experiments/entry15/build-helper`
- Layer: `experimental-build-tool`
- Status: `experimental`

## Role

Host build helper proving downstream linker arguments must originate in the product build script.

## Owns

Emit -Tlinkall.x from the downstream build.rs.

## Does not own

No production Board, boot-mode USB selection, physical storage, Wi-Fi,
OTA or hardware qualification. Not a framework API to integrate into products.

## Architecture position

Isolated experimental workspace; no changes to production workspace membership.
See [experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md). All crates are unpublished.

## Public contracts

emit() prints the linker argument; this crate is a host build dependency, not embedded code.

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

The entry task never completes in this fixture. A 96-KiB heap is initialized,
but no production resource budget or runtime heap/stack high-water mark is validated. The panic handler belongs to the fixture. Future sizes
are fixture-specific, not StreamBeWI memory requirements. Chip support is S3 only.
The negative async-main mode is expected to fail and is not a production feature.

## Related components

[Experiment instructions](https://github.com/iobewi/iobewi/blob/feat/15-entry-experiments/tools/experiments/entry15/README.md), issue #15 and proposed ADR-0015 in PR #16.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
