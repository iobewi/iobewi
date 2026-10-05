# Agent Context — iobewi-firmware-slots

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-firmware-slots`
- Path: `firmware/slots`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Logical names and indices for the existing two-slot resident application layout.

## Owns

Define ota_0/ota_1, their opposite/index/name mappings and the exposed layout identifier.

## Does not own

Physical partition addresses, per-slot metadata, boot trust, Workload slots and generalized kernel/userspace/recovery roles.

## Architecture position

Allocation-free portable vocabulary consumed by boot and ESP OTA mapping.

## Public contracts

`AppSlot::{Ota0,Ota1}`, `from_name`, `as_str`, `other`, `index`, `from_index`; `SLOT_COUNT = 2` and `PARTITION_LAYOUT = "embewi-ab-v1"`.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

`cargo test -p iobewi-firmware-slots` checks exact names, indices, opposite mapping and layout identifier.

## Known limitations

Only ota_0 and ota_1 are recognized; factory and additional names/indices return None. Changing these identifiers changes the OTA compatibility contract.

## Related components

`firmware/boot`, `firmware/esp32`, `firmware/update`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
