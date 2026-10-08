# ADR-0017 — Soft access point as an esp-radio mode behind a portable port

Status: proposed.
Related: issue #31. The qualification experiment on PR #32 (ADR-0016, proposed) studies a
different question, an access point toggled without interrupting the station, and is not a
prerequisite of this decision.

## Context and evidence

Provisioning a device that has no Wi-Fi credentials needs a local network the user can join:
an access point served by the device, with an address service and a small page served by the
product. `esp-radio 1.0.0-beta.1` supports station, access point and station + access point
(`Config::Station`, `Config::AccessPoint`, `Config::AccessPointStation`), each interface as its
own network device (`Interface::station()`, `Interface::access_point()`) that can feed its own
`embassy-net` stack. Reading the pinned source (`WifiController::set_config`): when the new mode
differs from the current one it calls `esp_wifi_stop`, then sets the mode and restarts; when the
mode is unchanged it applies only the changed interface configuration. A failed
`set_config` resets the mode to NULL and stops the radio.

The existing portable contract is station-only: `WifiTransport` says "online" means associated
and configured by DHCP.

## Decision

1. `net/wifi/core` gains a separate port, `WifiAccessPoint`, and a validated
   `AccessPointConfig` (SSID 1 to 32 bytes, WPA2 passphrase 8 to 63 bytes, channel 1 to 13;
   an open access point is not representable and `Debug` redacts the passphrase).
   `WifiTransport` is unchanged.
2. A platform implements both ports on the same object: one radio, one owner. No second
   independently mutable controller.
3. `net/wifi/manager` only delegates (`start_access_point`, `stop_access_point`,
   `is_access_point_active`, `access_point_handle`). When to open the access point and for how
   long, its SSID and passphrase, and what is served on it are product policy. Nothing is
   written to config-space.
4. `drivers/net/wifi/esp32` implements the port on `WifiManager`, opt-in through
   `with_access_point(resources)`: a second stack on `Interface::access_point()` with a static
   address, and the DHCP server of the external `edge-dhcp` crate over `edge-nal-embassy`, run
   only while the access point is active. The access point always runs as station + access
   point; while it is active, `connect` re-applies the station configuration in that same mode.
5. **A mode change restarts the radio and interrupts the station; this is accepted.** Starting
   or stopping the access point from a station-only radio drops the station link. The portable
   manager's reconnection loop recovers it; the documented sequence is: drop `maintain`, change
   the access point, call `maintain` again. The port's documentation states this.

## Scope and non-goals

- Provisioning an unconfigured device, and reprovisioning with an accepted interruption, are
  served. Opening the access point during streaming without interrupting it is not provided and
  not promised. The experiment of PR #32 (a dormant access point reconfigured in the same mode)
  remains the only candidate for that and is qualified separately.
- The product, not the driver, serves HTTP on `access_point_handle()` and stops it when it
  stops the access point. Plain HTTP on that network is an exception to the TLS-only management
  policy and must be limited to it.
- Wiring a second `StackResources` through `Board`, `entry` and their resource budget is a
  separate change; this decision adds no product dependency to those crates.
- No captive DNS or portal.

## Alternatives considered

- **A dormant access point toggled in the same mode** to avoid the restart: kept as an
  experiment (PR #32), not adopted, because its hardware result is not established and it
  leaves the access point radio, beacons and channel coupling permanently on.
- **Writing the DHCP server in the repository:** rejected; `edge-dhcp` is a maintained,
  allocation-free server whose releases match the repository's `embassy-net 0.9` and
  `embassy-time 0.5`.
- **Folding the access point into `WifiTransport`:** rejected; its "online" semantics are about
  a station lease and would be overloaded.

## Consequences and open validation

- Two new portable test sets (core, manager) and an ESP compile check on S3 and C3.
- New dependencies in the ESP driver: `edge-dhcp`, `edge-nal`, `edge-nal-embassy`,
  `embassy-futures`, `embassy-sync`, `static_cell`. The `board15` downstream link still builds
  and passes its ELF inspection with the new lockfile; the size effect on products that never call the
  access point was not measured.
- **Hardware acceptance is partial.** One run on an ESP32-S3 (see `examples/wifi/ap-setup`)
  observed association and lease, the product's page, provisioning with the access point up,
  the stop (as logged by the firmware) and the station's reconnection through `maintain`; the
  access point followed the router's channel during the join. A wrong passphrase was refused
  (`FourWayHandshakeTimeout`), left the access point up for a retry, and the right one then
  connected. Still to verify, as listed in the driver README: that the access point is really
  gone from the air and DHCP silent after the stop (the runs only show the firmware's own
  log), unchanged behavior with no access point configured, heap before, during and after
  repeated cycles, and ESP32-C3. `BG-ESP-S3` applies; the complete gate needs a board.
