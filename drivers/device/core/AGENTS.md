# Agent Context — iobewi-device

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-device`
- Path: `drivers/device/core`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Portable `no_std + alloc` contracts for hardware identity and static metadata.

## Owns

- Separate opaque identity from optional MAC and static platform facts.
- Define the framework's MAC-derived identifier convention.

## Does not own

No HAL/eFuse access, resource initialization, application naming prefix, OTA
partition metadata or identity authentication.

## Architecture position

Portable contracts at `drivers/device/core`; platform adapters implement the
traits. Product policy consumes these contracts without knowing the HAL.

## Public contracts

| API | Contract |
| --- | --- |
| `DeviceIdentity::hardware_id() -> String` | Hardware-derived opaque identifier; application must not assume a MAC/serial representation |
| `DeviceIdentity::mac_address() -> Option<[u8; 6]>` | Optional link-layer address; default implementation returns `None` |
| `DeviceMetadata::chip_name() -> &'static str` | Static platform name supplied by implementation |
| `DeviceMetadata::ram_size() -> u32` | Platform-reported RAM size; not current free heap or stack headroom |
| `hardware_id_from_mac([u8; 6]) -> String` | Last three MAC bytes, lowercase zero-padded hex, no prefix: `aa:bb:cc:0a:0b:ff` becomes `0a0bff` |

Identity and metadata are separate traits; a provider need not implement both.

## Invariants

- [INV-001](../../../INVARIANTS.md): portable code must not depend on platform implementations.

## Modification context

### Lifecycle

The contracts acquire no resource or initialization state. Implementations own
hardware access and preconditions. The MAC helper allocates its returned string;
embedded callers must provide an allocator. Methods return no `Result`; an absent
MAC is represented by `None`, and allocation failure follows the caller's allocator.

## Required validation

`cargo test -p iobewi-device` checks MAC formatting and independent capability values.
No hardware is required for these contract tests.

## Known limitations

A 24-bit MAC suffix is not a globally unique identifier or security credential.
The metadata contract does not distinguish internal RAM from PSRAM; consult the
adapter's definition. No network-interface selection or persisted application ID exists here.

## Related components

- [ESP identity and metadata](../esp32/README.md)
- [Runtime diagnostics](../../diagnostics/core/README.md)
- [Product integration](../../../docs/product-integration.md)

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
