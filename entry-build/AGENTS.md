# Agent Context — iobewi-entry-build

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-entry-build`
- Path: `entry-build`
- Layer: `build-tool`
- Status: `implemented`

## Role

Host-side linker policy for the first framework entry target.

## Owns

Emit the required linkall.x linker argument and reject an unsupported Cargo TARGET.

## Does not own

No firmware initialization, HAL access or resource allocation.

## Architecture position

Called by the consuming firmware build.rs; the firmware owns its workspace and depends on iobewi-entry for runtime composition.

## Public contracts

emit() accepts xtensa-esp32s3-none-elf and emits Cargo linker directives; another target causes a build error.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Modification context

See the canonical README and implementation.

## Required validation

Build and link tools/experiments/board15 using its separate product workspace.

## Known limitations

Only the S3 native-USB profile is supported initially. This helper does not install the ESP toolchain.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
