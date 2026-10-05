---
layer: portable-contract
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-firmware-slots

## Summary

Logical names and indices for the existing two-slot resident application layout.

## Responsibilities

Define ota_0/ota_1, their opposite/index/name mappings and the exposed layout identifier.

## Non-responsibilities

Physical partition addresses, per-slot metadata, boot trust, Workload slots and generalized kernel/userspace/recovery roles.

## Architecture

Allocation-free portable vocabulary consumed by boot and ESP OTA mapping.

## Public API

`AppSlot::{Ota0,Ota1}`, `from_name`, `as_str`, `other`, `index`, `from_index`; `SLOT_COUNT = 2` and `PARTITION_LAYOUT = "embewi-ab-v1"`.

## Invariants

- `INV-001`

## Validation

`cargo test -p iobewi-firmware-slots` checks exact names, indices, opposite mapping and layout identifier.

## Known limitations

Only ota_0 and ota_1 are recognized; factory and additional names/indices return None. Changing these identifiers changes the OTA compatibility contract.

## Related components

`firmware/boot`, `firmware/esp32`, `firmware/update`.
