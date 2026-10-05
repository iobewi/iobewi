---
layer: platform-driver
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-usb

## Summary

ESP32-S3 native USB FS device-driver construction.

## Responsibilities

Bind USB_FS with GPIO20 D+ and GPIO19 D- and a caller-owned OUT buffer.

## Non-responsibilities

No USB identity, MSC class, readiness policy, JTAG handover or logging.

## Architecture

The consuming platform BootIoFactory calls this constructor only in a MassStorage boot. No JTAG driver is constructed in that branch.

## Public API

device(USB_FS, GPIO20, GPIO19, static_out_buffer) returns an Embassy-compatible Driver; Driver is reexported for platform composition.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Validation

Compile and link the Board fixture in both modes; run BG-USB-MSC and reset/PHY checks on hardware.

## Known limitations

S3 only. Compilation cannot prove enumeration, warm-reset PHY state or host compatibility.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.
