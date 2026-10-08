# Agent Context — wifi-ap-setup-example

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `wifi-ap-setup-example`
- Path: `examples/wifi/ap-setup`
- Layer: `example`
- Status: `validation`

## Role

Hardware-agnostic example and hardware test method: provision Wi-Fi through the device's own soft access point (ADR-0017). One crate; the chip is a Cargo feature (`esp32s3`) of the framework's entry crate, chosen at compile time, and no source file names a chip.

## Owns

- The product entry `run<B: Board>(board)` and its `BOARD_RESOURCES`, started by the framework's `entry!` (ADR-0015). The access point is reached through `Board` with the bound `B::Wifi: WifiAccessPoint<NetworkHandle = embassy_net::Stack<'static>>`; the configuration is the board's config space (`ConfigManager` over `B::Config`), so the example touches no flash driver.
- The boot decision of a product: saved credentials mean joining the network directly with no access point; none (or a `maintain` that ends with `NotProvisioned`) means the setup path.
- The setup path: scan, open the access point, serve a form that lists the scanned networks strongest first, call the manager's `provision` with the access point still up, keep the answer readable for a few seconds, close the page, stop the access point, then `maintain`.
- Send the captured log records (`iobewi-log`) to the board's first serial port.

## Does not own

- Names no chip, HAL, flash driver or board pin. The library depends on portable crates (`iobewi-board`, `iobewi-wifi-*`, `iobewi-config-space`, `iobewi-http-server`, `iobewi-net-io`, `iobewi-log`), on the hardware-independent `embassy-net`, `embassy-time`, `embassy-sync`, `embassy-futures`, `picoserve`, `serde` and `log`, and on `iobewi-esp-tcp`, an `embassy-net` listener with no HAL dependency whose name is historical.
- Not a product: the page is plain HTTP on the access point network (the exception described in ADR-0017) with no authentication beyond the access point's WPA2 passphrase; no captive portal, no HTTPS. The access point's passphrase is a test value (`123456789`, overridable with `SETUP_AP_PASSWORD` at build time); a product needs a unique secret.
- There is no factory-reset control: reflashing the merged image rewrites the NVS area and starts over.

## Architecture position

Path: `examples/wifi/ap-setup`. Layer: **example**; an independent workspace (its `esp32s3` feature needs the Xtensa target, which a workspace-wide `--all-features` check cannot satisfy).

- `src/lib.rs`: `run<B: Board>`, `BOARD_RESOURCES`, and the sequence `run_product(transport, space, &access_point, page)`, generic over a transport that is both `WifiTransport` and `WifiAccessPoint`, a `ConfigBackend`, and a `PageListener<Handle>`: a small lending trait through which the page gets a `ConnectionListener` on the access point's network handle.
- `src/main.rs` (built only with `--features esp32s3`): `entry_api::entry!(wifi_ap_setup_example::run)` and nothing else.
- `build.rs`: emits the entry descriptor only when the hardware feature is on.

The page is served only while the setup runs; leaving the setup drops the listener before the access point is stopped.

## Public contracts

`run`, `BOARD_RESOURCES`, `run_product`, `PageListener`. The page, its routes (`/`, `/networks`, `/scan`, `/status`, `/connect`) and the state shared with the HTTP handlers are private.

## Invariants

- `INV-001`: nothing platform-specific is reachable from this crate; the hardware enters through `Board`.
- `INV-004`/`INV-005`: the single flash owner and the NVS backend belong to the ESP Board, not to this example.

## Modification context

See the canonical README and implementation.

## Required validation

Host: `cargo test --manifest-path examples/wifi/ap-setup/Cargo.toml` (best-first ranking, the list format, that no byte of a neighbour's SSID can break the line structure, the status words the page polls for). Firmware: `bash examples/wifi/ap-setup/run.sh` (also a CI link check).

Hardware method, with a board, a 2.4 GHz Wi-Fi network you can type credentials for, a phone or PC, and a serial capture (UART0 at 115200):

1. Flash the merged image at offset 0 (for example through ESP Web Tools, or `espflash save-image --merge` then `espflash write-bin 0x0`). Start the serial capture first, to a file.
2. Join `IOBEWI-Setup`. Expected: association, a lease in `172.23.241.2` to `.5`, and the page at `http://172.23.241.1/`. A phone may report no internet (there is no gateway or DNS) and may leave after about 20 s: turn mobile data off.
3. Submit a wrong password. Expected: `connection failed`, the page says so, the access point stays up.
4. Submit the right one. Expected: `provisioned, station connected, ip=...`; the page shows the address for 8 s, then the access point stops (the radio restarts) and `maintain` reconnects: `station READY`.
5. Reset or power cycle: the device must read the saved credentials, skip the access point and reach `station READY` again by itself.
6. To start over, reflash.

Record per run: the capture, whether the page loaded, the lease, the time from submit to `provisioned`. Acceptance for ADR-0017 on hardware: association and lease; the page reachable; wrong credentials leave the access point usable; right credentials connect the station while the access point is up; after the stop the station reconnects through `maintain` and the access point is no longer visible.

**Hardware runs so far** (one ESP32-S3, one router with several access points on the same SSID, one client; they used an earlier layout of this example, whose logic is the same):

1. First run: first scan 20 access points / 7 networks in about 1.5 s; the access point active about 1.8 s after its start; a client joined, loaded the page, rescanned with the access point up (19 access points / 7 networks) and submitted the real network; the station associated in about 4 s on channel 11 at -45 dBm, reported as the pinned strongest access point; after the 8 s hold the access point stopped and `maintain` reconnected in about 4 s (`ready after 0 failed attempt(s)`, same address). The page showed a `NetworkError` after the submit although the connection succeeded (the answer was lost while the radio changed channel; see Known limitations).
2. Wrong passphrase then the right one: the first attempt ended after 7.6 s with `FourWayHandshakeTimeout`, logged as `refused the credentials`; the access point stayed up and the client came back; the retry connected in 4.6 s and the sequence ended as above.
3. NVS backend: the first boot reported `saved credentials: no`, ran the setup and committed `generation=1`; after a reset it reported `saved credentials: yes`, skipped the access point and reached `station READY` with the same address, so the persistence through the repository's config space works. That boot joined at **channel 1, -67 dBm, chosen by the radio** although channel 11 at -44 dBm had been joined before: with no pin, the radio's own choice among access points of one SSID is unreliable. The driver now runs a directed scan for the SSID and pins the strongest before joining.

Not yet exercised on hardware: this single-crate layout on the Board/entry path (the runs above used a hand-written composition), the boot with saved credentials after the directed-scan change, that the access point is really gone from the air after the stop (only the firmware's own log was read), repeated start/stop cycles with a heap measurement, ESP32-C3, and a boot with no access point configured. `esp_wifi_internal_tx returned error: 12309` (`ESP_ERR_WIFI_NOT_ASSOC`) appears repeatedly during and after join attempts; it was harmless in every run and its source is not diagnosed.

## Known limitations

Joining the network moves the radio to the router's channel (here 6 to 11) and the access point follows, so the client's link to the page can drop for a moment: the page treats a failed request as "in progress" and polls the status, and the sequence waits 800 ms before connecting so the answer leaves first. A rescan while a client is connected runs the station's scan on the shared radio, which can pause the access point's traffic for a moment. Starting and stopping the access point restart the radio (ADR-0017). The station reconnection after the stop depends on the driver's disconnect and configuration-down waits, which have no timeout of their own; if no `station READY` follows `access point stopped`, look there first. The state shared with the HTTP handlers is process-wide: one setup at a time. The sequence and the handlers are not covered by host tests. Logs go to the board's first serial port, which on the ESP32-S3 profile is UART0, sharing it with anything else the product writes there.

## Related components

- `board`, `entry`, `arch/esp32/runtime` (the Board and its resource request)
- `net/wifi/core`, `net/wifi/manager`, `drivers/net/wifi/esp32`
- `docs/decisions/ADR-0017-wifi-access-point-port.md`, `docs/decisions/ADR-0015-board-entry.md`

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
