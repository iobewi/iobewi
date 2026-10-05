---
layer: portable-contract
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-firmware-image

## Summary

Portable artifact digest representation and host-testable ESP application-image validation.

## Responsibilities

Parse SHA-256 textual digests and validate ESP headers, segments, integrity and address constraints through a supplied reader and memory map.

## Non-responsibilities

Flash ownership, partition discovery, slot selection, OTA state and native IWNI Workload image parsing.

## Architecture

Pure image semantics below firmware boot/update; ESP image rules are data-driven and do not require a HAL.

## Public API

`Digest`, `parse_digest`, and alloc-gated `format_digest`; module `esp` exports `Read`, `MemoryMap`, `Verify`, `Image`, `Segment`, `ImageError` and `validate`.

## Invariants

- `INV-001`

## Validation

`cargo test -p iobewi-firmware-image --features alloc` covers digest and ESP image validation.

## Known limitations

Caller supplies the correct chip memory map and reader bounds. Digest integrity is not signature authentication or memory isolation. ESP application images and IWNI Workload images are different formats. Formatting requires feature alloc.

## Related components

`firmware/boot`, `arch/esp32/boot`; `workload/image` owns IWNI.
