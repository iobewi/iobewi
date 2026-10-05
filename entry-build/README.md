---
layer: build-tool
status: implemented
invariants: []
gates: []
---

# iobewi-entry-build

## Summary

Host-side linker policy for the first framework entry target.

## Responsibilities

Emit the required linkall.x linker argument and reject an unsupported Cargo TARGET.

## Non-responsibilities

No firmware initialization, HAL access or resource allocation.

## Architecture

Called by the consuming firmware build.rs; the firmware owns its workspace and depends on iobewi-entry for runtime composition.

## Public API

emit() accepts xtensa-esp32s3-none-elf and emits Cargo linker directives; another target causes a build error.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Validation

Build and link tools/experiments/board15 using its separate product workspace.

## Known limitations

Only the S3 native-USB profile is supported initially. This helper does not install the ESP toolchain.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.
