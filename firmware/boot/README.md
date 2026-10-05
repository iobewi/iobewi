---
layer: portable-contract
status: implemented
invariants:
  - INV-001
  - INV-009
  - INV-017
  - INV-021
gates: []
---

# iobewi-firmware-boot

## Summary

Pure transactional EWBT otadata format and A/B boot decision logic.

## Responsibilities

Validate committed entries, plan boot/pending/rollback transitions and plan activation, confirmation or rejection while preserving the last Valid fallback.

## Non-responsibilities

Flash I/O, image loading, partition discovery, OTM1 persistence and Workload activation.

## Architecture

Allocation-free portable boot policy executed by the platform bootloader and resident firmware adapters.

## Public API

`Entry`, `Raw`, `Decoded`, `Write`, `Op`, `Plan`, `Boot`, `Halt`; `decode`, `plan_boot`, `activate`, `confirm`, `reject`, `update_target`, `slot_of` and entry queries. `Write::ops` specifies erase, body program, then a separate commit-word program.

## Invariants

- `INV-001`
- `INV-009`
- `INV-017`
- `INV-021`

## Validation

`cargo test -p iobewi-firmware-boot` includes adversarial interrupted flash-command tests. BG-AGENT-OTA validates execution of these plans on hardware.

## Known limitations

EWBT deliberately rejects legacy ESP-IDF-format entries. Executor must verify the body before committing. Blank/corrupt first-boot metadata can seed slot 0 only if bootable; rejected candidates do not silently reseed. Caller supplies valid nonzero slot counts and valid target indices. This is a plan, not executed flash durability.

## Related components

`firmware/image`, `firmware/slots`, `firmware/esp32`, `arch/esp32/boot`, `bootloader/esp`.
