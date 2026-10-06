# Agent Context — iobewi-esp-usb

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-usb`
- Path: `drivers/usb/esp32`
- Layer: `platform-driver`
- Status: `implemented`

## Role

ESP32-S3 native USB FS device-driver construction.

## Owns

Bind USB_FS with GPIO20 D+ and GPIO19 D- and a caller-owned OUT buffer.

## Does not own

No USB identity, MSC class, readiness policy, JTAG handover or logging.

## Architecture position

The consuming platform BootIoFactory calls this constructor only in a MassStorage boot. No JTAG driver is constructed in that branch.

## Public contracts

device(USB_FS, GPIO20, GPIO19, static_out_buffer) returns an Embassy-compatible Driver; Driver is reexported for platform composition.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Modification context

See the canonical README and implementation.

## Required validation

Compile and link the Board fixture in both modes; run BG-USB-MSC and reset/PHY checks on hardware.

## Known limitations

S3 only. Compilation cannot prove enumeration, warm-reset PHY state or host compatibility.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
