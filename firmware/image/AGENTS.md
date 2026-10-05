# Agent Context — iobewi-firmware-image

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-firmware-image`
- Path: `firmware/image`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Portable artifact digest representation and host-testable ESP application-image validation.

## Owns

Parse SHA-256 textual digests and validate ESP headers, segments, integrity and address constraints through a supplied reader and memory map.

## Does not own

Flash ownership, partition discovery, slot selection, OTA state and native IWNI Workload image parsing.

## Architecture position

Pure image semantics below firmware boot/update; ESP image rules are data-driven and do not require a HAL.

## Public contracts

`Digest`, `parse_digest`, and alloc-gated `format_digest`; module `esp` exports `Read`, `MemoryMap`, `Verify`, `Image`, `Segment`, `ImageError` and `validate`.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

`cargo test -p iobewi-firmware-image --features alloc` covers digest and ESP image validation.

## Known limitations

Caller supplies the correct chip memory map and reader bounds. Digest integrity is not signature authentication or memory isolation. ESP application images and IWNI Workload images are different formats. Formatting requires feature alloc.

## Related components

`firmware/boot`, `arch/esp32/boot`; `workload/image` owns IWNI.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
