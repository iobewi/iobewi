# Agent Context — iobewi-ota

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-ota`
- Path: `firmware/update`
- Layer: `portable-service`
- Status: `implemented`

## Role

Portable `no_std` Agent firmware-update transaction service: prepare, resumable streaming write, SHA-256 verification, durable OTM1 identity, activation/reconciliation and confirm-or-rollback orchestration.

## Owns

- Own portable Agent OTA transaction semantics and restart-safe reconciliation.
- With the optional `runtime` feature, own the live upload session, boot reconciliation, confirmation gate and deadline.

## Does not own

- Does not own HTTP routes; those live in `net/http/ota`.
- Does not own ESP partition lookup/flash execution or the second-stage bootloader.
- Does not own Workload OTM2 lifecycle.

## Architecture position

Portable Agent OTA service. OTM1 remains the Agent firmware metadata contract; ESP-specific storage/partition execution is implemented below this layer.

## Public contracts

Public transaction/runtime APIs are exported by the crate. Optional features include `runtime` and `config-space`. OTM1 on-device compatibility must be preserved.

## Invariants

- `INV-001`
- `INV-003`
- `INV-009`
- `INV-017`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-AGENT-OTA`

## Known limitations

Refactoring low-level NOR/partition erase abstractions is intentionally deferred because it would requalify the long-validated Agent OTA path.

## Related components

- `net/http/ota`
- `firmware/boot`
- `firmware/image`
- `firmware/slots`
- `firmware/esp32`
- `docs/dual-ota.md`

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
