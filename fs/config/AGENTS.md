# Agent Context — iobewi-config-space

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-config-space`
- Path: `fs/config`
- Layer: `portable-service`
- Status: `implemented`

## Role

Portable `no_std`, hardware-agnostic configuration ownership, quota and opaque replacement-commit layer.

## Owns

- Own unique configuration-space ownership, reservation/admission control, per-space payload limits, generations and capability-handle isolation.
- Define the generic `ConfigBackend` persistence contract while leaving schemas opaque.

## Does not own

- Does not know Wi-Fi SSIDs, certificates, tokens, GPIOs, flash sectors or NVS namespaces.
- Does not own component serialization schemas/migrations or provisioning policy.
- Does not own backend-specific capacity accounting and physical atomicity.

## Architecture position

Portable configuration service. Components claim an opaque space with a budget; platform backends translate logical reservations into storage-specific capacity and atomic replacement guarantees.

## Public contracts

Callers claim a space, then load/commit an opaque value through the returned `ConfigSpace` capability. A successful claim is a boot-lifetime reservation guarantee.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-STORAGE`

## Known limitations

A logical payload byte is not assumed to equal one physical storage byte; backend reservation accounting determines whether a claim can be guaranteed.

## Related components

- `fs/nvs/core`
- `fs/nvs/config-esp32`

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
