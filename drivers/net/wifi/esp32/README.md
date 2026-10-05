---
layer: platform-adapter
status: implemented
invariants:
  - INV-001
gates:
  - BG-ESP-S3
---

# iobewi-esp-wifi

## Summary

Reusable `no_std` ESP Wi-Fi station transport built on `esp-radio` and `embassy-net`.

## Responsibilities

- Own lazy station initialization, scanning/strongest-BSSID selection, association, DHCP and the Embassy network runner.
- Expose the configured IP stack through the portable Wi-Fi transport boundary.

## Non-responsibilities

- Does not own credential persistence or NVS layout.
- Does not own provisioning policy, TLS, HTTP, heartbeat/log services, OTA or application supervision.

## Architecture

Platform Wi-Fi adapter. `net/wifi/core` defines the portable transport/provisioning capabilities and `net/wifi/manager` owns durable credentials and retry/reprovision policy.

## Public API

Features `esp32c3` and `esp32s3` select the chip. The caller supplies `StackResources<N>` so socket capacity remains a composition decision.

`WifiManager::new` retains the peripheral and stack resources. `scan` and `connect` lazily initialize the radio, DHCP stack and network runner. `network_handle()` returns the stack handle after IPv4 configuration becomes available.

## Lifecycle

The first initialization consumes the supplied resources; subsequent connections reuse the same stack. Calling `connect` reconfigures the station even when credentials are unchanged: it disconnects an existing association, waits for the old DHCP configuration to disappear, applies credentials, then associates and waits for DHCP.

The handle identifies the reused stack, not permanent link availability. Product composition may publish it to consumers through the portable manager's `LinkObserver::ready`; `link_down` reports configuration loss. The portable manager retains reconnection policy ownership.

## Invariants

- `INV-001`

## Validation

- `BG-ESP-S3`

## Known limitations

No chip is selected by default; hardware validation is required when radio/HAL versions or connection mechanics change. Association and the subsequent DHCP wait each have a 20-second timeout; the earlier disconnect and configuration-down waits are not covered by those timeouts.

## Related components

- `net/wifi/core`
- `net/wifi/manager`

