# Agent Context — iobewi-esp-wifi

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-wifi`
- Path: `drivers/net/wifi/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

Reusable `no_std` ESP Wi-Fi station transport built on `esp-radio` and `embassy-net`.

## Owns

- Own lazy station initialization, scanning/strongest-BSSID selection, association, DHCP and the Embassy network runner.
- Expose the configured IP stack through the portable Wi-Fi transport boundary.

## Does not own

- Does not own credential persistence or NVS layout.
- Does not own provisioning policy, TLS, HTTP, heartbeat/log services, OTA or application supervision.

## Architecture position

Platform Wi-Fi adapter. `net/wifi/core` defines the portable transport/provisioning capabilities and `net/wifi/manager` owns durable credentials and retry/reprovision policy.

## Public contracts

Features `esp32c3` and `esp32s3` select the chip. The caller supplies `StackResources<N>` so socket capacity remains a composition decision.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-ESP-S3`

## Known limitations

No chip is selected by default; hardware validation is required when radio/HAL versions or connection mechanics change.

## Related components

- `net/wifi/core`
- `net/wifi/manager`

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
