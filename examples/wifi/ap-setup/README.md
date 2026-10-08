---
layer: example
status: validation
invariants:
  - INV-001
gates: []
---

# wifi-ap-setup-example

## Summary

Hardware-agnostic example: provision Wi-Fi through the device's own soft access point (ADR-0017). Portable product logic that consumes ports only; a target composition package (for the ESP32-S3, `esp32s3/`) owns the hardware and calls it.

## Responsibilities

- The boot decision of a product: saved credentials mean joining the network directly with no access point; none (or a `maintain` that ends with `NotProvisioned`) means the setup path.
- The setup path: scan, open the access point, serve a form that lists the scanned networks strongest first, call the manager's `provision` with the access point still up, keep the answer readable for a few seconds, close the page, stop the access point, then `maintain`.
- Show how a product uses the portable pieces: `WifiTransport` + `WifiAccessPoint` (`net/wifi/core`), `WifiManager` and `is_provisioned` (`net/wifi/manager`), `ConfigSpace` (`fs/config`), `HttpRouter` and `serve_forever_io` (`net/http/server`), `ConnectionListener` (`net/io`).

## Non-responsibilities

- Names no chip, HAL, flash driver, network stack or TCP implementation: no `esp-*`, `embassy-net` or `iobewi-esp-*` dependency. Time, signals and `select` come from the hardware-independent `embassy-time`, `embassy-sync` and `embassy-futures` (the target supplies the time driver and the critical-section implementation); logging is the `log` facade.
- Not a product: the page is plain HTTP on the access point network (the exception described in ADR-0017) with no authentication beyond the access point's WPA2 passphrase; no captive portal, no HTTPS.
- Does not choose the access point's SSID, passphrase or channel, nor how a listener reaches the access point's network: the composition does.

## Architecture

Path: `examples/wifi/ap-setup`. Layer: **example**; a member of the root workspace.

`run(transport, space, &access_point, page) -> !` is generic over a transport that is both `WifiTransport` and `WifiAccessPoint` (with a displayable address), a `ConfigBackend`, and a `PageListener<Handle>`: a small lending trait through which the target builds a `ConnectionListener` on the access point's network handle (so a target can keep the buffers it owns and the setup can run again). The page is served only while the setup runs; leaving the setup drops the listener before the access point is stopped.

## Public API

`run` and `PageListener`. Everything else is private: the page, its routes (`/`, `/networks`, `/scan`, `/status`, `/connect`), and the shared state between the HTTP handlers and the sequence.

## Invariants

- `INV-001`: nothing platform-specific is reachable from this crate.

## Validation

`cargo test -p wifi-ap-setup-example` (host): best-first ranking, the list format, that no byte of a neighbour's SSID can break the line structure, and the status words the page polls for. The sequence itself needs a transport and a listener; it is exercised on hardware by the composition (`esp32s3/README.md` holds the method and the recorded runs) and compiled by `cargo check --workspace`.

## Known limitations

The state shared with the HTTP handlers is process-wide (statics): one setup at a time. The sequence and the HTTP handlers are not covered by host tests. Joining the network moves the radio to the router's channel, so the client's link to the page can drop for a moment (ADR-0017); the page tolerates it.

## Related components

- `examples/wifi/ap-setup/esp32s3` (the ESP32-S3 composition)
- `net/wifi/core`, `net/wifi/manager`, `drivers/net/wifi/esp32`
- `docs/decisions/ADR-0017-wifi-access-point-port.md`, `docs/decisions/ADR-0014-product-composition.md`
