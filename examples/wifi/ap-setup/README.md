---
layer: example
status: validation
invariants:
  - INV-001
gates:
  - BG-ESP-S3
---

# wifi-ap-setup-example

## Summary

ESP32-S3 example and hardware test method for provisioning a device through its own soft access point (ADR-0017): the device opens `IOBEWI-Setup`, serves a form, connects the station with the real `WifiManager::provision`, then closes the access point.

## Responsibilities

- Show the intended product sequence on the production code, nothing re-implemented: `WifiManager::start_access_point`, a page served on `access_point_handle()`, `provision` with the access point still up, `stop_access_point`, then `maintain` from the saved credentials.
- Serve as the hardware test of the access point, its DHCP service and the station together: one run exercises association, lease, HTTP, provisioning and the reconnection after the radio restart.

## Non-responsibilities

- Not a product: credentials are kept in RAM only (a power cycle starts unconfigured again) and nothing is written to flash.
- The page is plain HTTP on the access point network, the exception described in ADR-0017; it has no authentication beyond the access point's WPA2 passphrase.
- No captive portal, no HTTPS.

## Architecture

Path: `examples/wifi/ap-setup`. Layer: **example**. Independent workspace; it depends by path on `iobewi-esp-wifi`, `iobewi-esp-tcp`, `iobewi-wifi-core`, `iobewi-wifi-manager`, `iobewi-config-space` and `iobewi-http-server`.

Sequence (UART lines are prefixed `SETUP:`):

0. A first scan runs before the access point exists (nobody is connected yet): `scan: N networks`.
1. Start: `access point ACTIVE: join "IOBEWI-Setup", open http://172.23.241.1/`.
2. A client joins with the passphrase (default `123456789`, test only; override with `SETUP_AP_PASSWORD` at build time), gets a lease from the built-in DHCP server and opens the page.
3. The form lists the scanned networks, strongest signal first (dBm and a 0 to 100 % bar, a lock for secured ones); choosing one fills the SSID field, which stays editable for hidden networks. "Actualiser" asks for a new scan (`GET /scan`, then `GET /networks` until `ready`). Network names are rendered as text only (neighbours' SSIDs are untrusted) and only counts reach the UART. The form posts SSID and password. The firmware logs `provisioning requested (ssid N bytes)` (never the SSID or password) and calls `provision` with the access point still up.
4. On success: `provisioned, station connected, ip=...`; the page shows the address for 8 s, the page task stops, the access point stops (the radio restarts) and `maintain` reconnects from the saved credentials: `station READY ip=...`.
5. On failure: `connection failed; access point stays up for a retry`.

## Public API

None. `run.sh [locked|latest]` builds `target/xtensa-esp32s3-none-elf/release/wifi-ap-setup-example`.

## Invariants

- `INV-001`: platform code stays in the ESP driver; the portable manager and contracts are used as they are.

## Validation

Build: `bash examples/wifi/ap-setup/run.sh` (also a CI link check). Hardware method, with a board, a 2.4 GHz Wi-Fi network you can type credentials for, a phone or PC, and a UART capture:

1. Flash the image (merged image at offset 0, for example through ESP Web Tools or `espflash save-image --merge` then `espflash write-bin 0x0`). Start the UART capture first, to a file.
2. Join `IOBEWI-Setup`. Expected: `JOINED`-level association, a lease in `172.23.241.2` to `.5`, and the page at `http://172.23.241.1/`. A phone may report no internet (there is no gateway or DNS) and may leave after about 20 s: turn mobile data off.
3. Submit a wrong password. Expected: `connection failed`, the page says so, the access point stays up.
4. Submit the right one. Expected the sequence in step 4 above and the station address on the UART; check the address on the router.
5. Power cycle: the device is unconfigured again.

Record per run: the UART capture, whether the page loaded, the lease, the time from submit to `provisioned`, and the heap if logged. Acceptance for ADR-0017 on hardware: association and lease; the page reachable; wrong credentials leave the access point usable; right credentials connect the station while the access point is up; after the stop the station reconnects through `maintain` and the access point is no longer visible. **No hardware run exists yet.**

## Known limitations

A rescan while a client is connected runs the station's scan on the shared radio, which can pause the access point's traffic for a moment (the page warns about it); the first scan avoids this. Starting and stopping the access point restart the radio (ADR-0017): the page is cut at the stop, which is why the success message is held for 8 s first. The station reconnection after the stop depends on the driver's disconnect and configuration-down waits, which have no timeout of their own; if the UART shows no `station READY` after `access point stopped`, that is the first place to look. The DHCP server (external `edge-dhcp`) hands out no gateway and no DNS.

## Related components

- `drivers/net/wifi/esp32`, `net/wifi/core`, `net/wifi/manager`
- `docs/decisions/ADR-0017-wifi-access-point-port.md`
