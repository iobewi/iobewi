---
layer: platform-adapter
status: implemented
invariants:
  - INV-004
  - INV-005
  - INV-009
  - INV-020
gates:
  - BG-AGENT-OTA
  - BG-STORAGE
  - BG-ESP-S3
---

# iobewi-esp-ota

## Summary

ESP resident OTA partition/storage and EWBT execution adapters.

## Responsibilities

Map logical app slots to discovered partitions, perform bounded NOR erase/write operations and execute EWBT metadata actions. With shared-flash, provide shared writers and runtime adapters.

## Non-responsibilities

Product identities, HTTP routes, remote physical-slot authority and the portable transaction model.

## Architecture

Platform implementation of firmware/update contracts using ESP partition/flash adapters. Product composition selects features and supplies the unique SharedFlash owner.

## Public API

`EspOtaPlatformMetadata`, `AppPartition`, `find_app_partition`, `erase_partition_range`, `EspArtifactStorage::new/new_pre_erased`, `FlashWriteError`, and `otadata`. Feature `shared-flash` exposes `shared_flash` (ArtifactWriter and boot operations) and `service` (EspBoot, EspBootRuntime, EspUploadWriter and reset/watchdog helpers).

## Invariants

- `INV-004`
- `INV-005`
- `INV-009`
- `INV-020`

## Validation

Build targets/esp32 with the selected chip/features; BG-AGENT-OTA and BG-STORAGE cover hardware boot and shared storage durability.

## Known limitations

Caller selects a safe target before erase. Scratch must hold an erase block. Durable writes advance by complete erase blocks; finish accepts a final partial block. Pre-erased storage requires the caller to have erased the range. ESP chip features are explicit; shared-flash is opt-in. Low-level APIs do not themselves enforce all product transaction/persistence policy.

## Related components

`firmware/update`, `firmware/boot`, `firmware/slots`, `drivers/flash/esp32`, `drivers/flash/partitions-esp32`.
