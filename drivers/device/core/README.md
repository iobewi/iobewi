---
layer: portable-contract
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-device

## Summary

Portable `no_std + alloc` contracts for hardware identity and static metadata.

## Responsibilities

- Separate opaque identity from optional MAC and static platform facts.
- Define the framework's MAC-derived identifier convention.

## Non-responsibilities

No HAL/eFuse access, resource initialization, application naming prefix, OTA
partition metadata or identity authentication.

## Architecture

Portable contracts at `drivers/device/core`; platform adapters implement the
traits. Product policy consumes these contracts without knowing the HAL.

## Public API

| API | Contract |
| --- | --- |
| `DeviceIdentity::hardware_id() -> String` | Hardware-derived opaque identifier; application must not assume a MAC/serial representation |
| `DeviceIdentity::mac_address() -> Option<[u8; 6]>` | Optional link-layer address; default implementation returns `None` |
| `DeviceMetadata::chip_name() -> &'static str` | Static platform name supplied by implementation |
| `DeviceMetadata::ram_size() -> u32` | Platform-reported RAM size; not current free heap or stack headroom |
| `hardware_id_from_mac([u8; 6]) -> String` | Last three MAC bytes, lowercase zero-padded hex, no prefix: `aa:bb:cc:0a:0b:ff` becomes `0a0bff` |

PinMetadata is an additional optional descriptive capability. Identity and metadata are separate traits; a provider need not implement both.

### Portable GPIO metadata

PinMetadata exposes a static slice of PinDescriptor and sparse lookup by PinId. Each descriptor names the adapter-local GPIO, its digital input/output capabilities and PinFunction alternatives with SignalDirection and an opaque selector name. PinId is not a physical header or package number. This optional capability grants no GPIO ownership, register access or pin configuration. A product requiring it adds a bound to B::Identity; Board does not impose it on all adapters.

## Lifecycle

The contracts acquire no resource or initialization state. Implementations own
hardware access and preconditions. The MAC helper allocates its returned string;
embedded callers must provide an allocator. Methods return no `Result`; an absent
MAC is represented by `None`, and allocation failure follows the caller's allocator.

## Invariants

- [INV-001](../../../INVARIANTS.md): portable code must not depend on platform implementations.

## Validation

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
