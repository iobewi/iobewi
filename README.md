# IOBEWI

IOBEWI is the company behind this portable Rust framework for embedded services. This repository contains the framework; the company name also covers its other projects.

> **Status:** HTTP, HTTPS, ConfigSpace and OTA provide separate portable crates. Service composition, OTA routes, platform ports beyond ESP and further application migrations remain in progress.

## Goal

`iobewi` defines how embedded applications obtain services through platform-independent contracts. An application such as [embewi-agent](https://github.com/iobewi/embewi-agent) supplies its own behavior and consumes those services. Platform adapters provide the hardware capabilities required to run them on ESP, RP2350, Teensy, or another supported target.

This repository is a Cargo workspace: each service lives in its own crate under
`services/` with its own `Cargo.toml`. Current members are [`iobewi-http-server`](net/http/server), the portable HTTP
dispatcher; [`iobewi-http-client`](net/http/client), outbound HTTP/WebSocket client primitives; and
[`iobewi-config-space`](fs/config), configuration ownership and quotas; and
[`iobewi-ota`](services/ota), transactional firmware updates.

The intended dependency direction is:

```text
embewi-agent (application)
    -> iobewi (service contracts and composition)
        -> services (OTA, HTTP, HTTPS, ConfigSpace, ...)
        -> platform implementations (arch/esp32, drivers/*/esp32, fs/nvs/esp32, firmware/esp32, bootloader/esp; future RP2350/Teensy)
            -> HAL, network and execution runtime
```

The application should not need an ESP-specific type to use OTA, HTTP, storage, or the watchdog. A service may require capabilities that a given board does not provide; the composition must make that requirement explicit.

## Components and ownership

| Component | Responsibility |
| --- | --- |
| `iobewi` | Portable service contracts, capability requirements, application bootstrap, and service composition. |
| `firmware/update` (`iobewi-ota`) | Firmware update transactions: prepare, streaming write, publish, activate, reconcile after restart, confirm-or-rollback orchestration (OTM1 metadata, bootstrap lifecycle). No HTTP, ESP or flash types. Package name is historical. |
| `firmware/image` (`iobewi-firmware-image`) | SHA-256 digest type and text form; ESP application image bootability validation (pure, host-testable). |
| `firmware/slots` (`iobewi-firmware-slots`) | The current A/B slot model: `AppSlot::{Ota0, Ota1}` and the `embewi-ab-v1` layout identifier. |
| `firmware/boot` (`iobewi-firmware-boot`) | Logical boot state: the EWBT `otadata` entry format and the power-cut-safe boot/activate/confirm/reject transitions. No hardware, no allocation. |
| `net/http/ota` (`iobewi-ota-http`) | The OTA HTTP routes (prepare / streaming write / activate); handlers call into `firmware/update`. |
| `net/http/server` (`iobewi-http-server`) | HTTP dispatcher (picoserve confined here), `IoSocket` adapter from any `net/io` connection, serve loops (`serve_forever_io`, `serve_forever_tls` for HTTPS). |
| `net/http/client` (`iobewi-http-client`) | Outbound JSON POST/response framing and, behind the `websocket` feature, the WebSocket client protocol over a connected stream. No picoserve. |
| `net/tls/service` (`iobewi-tls-service`) | Durable TLS identity/CA policy (TLS1 config), authenticated provisioning API, and the fail-closed secure outbound connector (`SecureConnector`) over the crypto and `net/tls/core` contracts; no ESP/MbedTLS type. |
| `crypto/core` (`iobewi-crypto-core`) | `TlsCrypto` contract: certificate/key/CA validation and identity generation. |
| `crypto/mbedtls` (`iobewi-crypto-mbedtls`) | MbedTLS implementation of `TlsCrypto` (PEM/X.509, P-256 identity, hooks); randomness injected via `crypto/rng`. Provisional ESP workspace: mbedtls-rs-sys does not build for the x86_64 host. |
| `net/wifi/core` (`iobewi-wifi-core`) | Wi-Fi contracts: `WifiTransport`, `WifiProvisioning`, `Network`; no config, radio or stack types. |
| `net/wifi/manager` (`iobewi-wifi-manager`) | Durable credentials (WFC1), reconnection, provisioning/reprovisioning policy over any `WifiTransport`. |
| `drivers/net/wifi/esp32` (`iobewi-esp-wifi`) | ESP station driver (esp-radio + embassy-net DHCP); implements `WifiTransport`, depends on the core only. |
| `arch/esp32/platform` (`iobewi-esp-platform`) | ESP SoC descriptors: chip ids and memory maps (`chips::{esp32c3, esp32s3}`). Pure, host-testable. |
| `arch/esp32/boot` (`iobewi-esp-boot`) | Second-stage boot hardware primitives per SoC (ROM flash/cache, flash-boot watchdog clear). No policy. |
| `arch/esp32/reset` (`iobewi-esp-reset`) | Physical SoC reset: digital-core `software_reset` and RTC-watchdog `arm_system_reset`. No executor, no firmware policy. |
| `arch/esp32/{c3,s3}/linker` | The bootloader memory maps (linker scripts), including the S3 D-cache/ROM-data layout rules and their `ASSERT`s. |
| `drivers/watchdog/esp32` (`iobewi-esp-watchdog`) | TIMG0 watchdog primitive (arm / feed / disable); the policy (PendingVerify) stays in `firmware/update`. |
| `drivers/flash/esp32` (`iobewi-esp-flash`) | The one physical ESP flash owner: `FlashStorage` built once, one `SharedFlash` mutex serializing every consumer. No NVS/OTA/ConfigSpace policy. |
| `drivers/flash/partitions-esp32` (`iobewi-esp-partitions`) | ESP-IDF partition-table lookup and partition-relative erase arithmetic. Policy-free. |
| `fs/nvs/core` (`iobewi-nvs-core`) | Chip-independent logic of NVS-backed ConfigSpace persistence: `CSM1` record framing, key rules, NVS entry accounting and the capacity formula. Host-tested. |
| `fs/nvs/esp32` (`iobewi-esp-nvs`) | NVS view over the shared flash (`esp-nvs` platform bridge). No ConfigSpace, no policy. |
| `fs/nvs/config-esp32` (`iobewi-esp-config-space`) | The ESP NVS implementation of `iobewi-config-space`'s `ConfigBackend` (locking the shared flash, calling `esp-nvs`); formulas and framing come from `fs/nvs/core`. |
| `firmware/esp32` (`iobewi-esp-ota`) | ESP storage adapter for `firmware/update`: slot -> partition mapping, `otadata` (EWBT) physical access, artifact storage over partitions, and the boot/upload wiring. |
| `drivers/device/core` (`iobewi-device`) | Device capability contracts (`DeviceIdentity`, `DeviceMetadata`) and the MAC-derived hardware-id convention (`hardware_id_from_mac`, host-tested). |
| `drivers/device/esp32` (`iobewi-esp-device`) | Reads the eFuse base MAC, chip name and RAM size; identity rule comes from the contract crate. |
| `drivers/indicator/core` (`iobewi-indicator`) | Logical status contract (`Status`, `StatusIndicator`, `StatusIndicatorCapabilities`); no colours, pins or timing. |
| `drivers/indicator/esp32` (`iobewi-esp-indicator`) | RMT/WS2812 status LED renderer (maps the abstract status to colour/blink) and its Embassy task. |
| `drivers/diagnostics/core` (`iobewi-runtime`) | `RuntimeDiagnostics` contract: stack headroom and heap free bytes. |
| `arch/esp32/runtime` (`iobewi-esp-runtime`) | Stack-painting high-water-mark (reads SP / linker stack symbols) and `esp-alloc` heap figures. |
| `net/io` (`iobewi-net-io`) | Low-level connection contracts: `Close`, `Connection`, `ConnectionListener`, outbound `Connector`; no protocol, TLS or platform types. |
| `net/tls/core` (`iobewi-net-tls-core`) | TLS network contracts: `SecureClientTransport`, `TlsListener` (a `ConnectionListener` yielding only TLS-handshaken connections), `TlsDialer`; no picoserve, config or ESP type. |
| `log/core` (`iobewi-log`) | Local log capture: bounded ring and the single global logger; no network dependency. |
| `log/stream` (`iobewi-log-stream`) | WebSocket forwarding of captured logs and reconnect policy over a supplied secure transport. |
| `time/core` (`iobewi-time`) | Unix epoch clock state (`now`, `is_set`, `wait`, `set_synced`), independent of any network stack. |
| `time/ntp` (`iobewi-ntp`) | SNTP synchronization over Embassy networking, feeding `iobewi-time`; server and timing policy are supplied at startup. |
| `fs/config` (`iobewi-config-space`) | Logical persistent configuration spaces, quotas and generations independent of the physical backend. |
| `bootloader/esp` (`iobewi-esp-bootloader`) | ESP second-stage bootloader: a platform executable composing `arch/esp32`, `firmware/boot` and `firmware/image` (own workspace and lockfile). |
| `targets/esp32` | Build/toolchain configuration of the ESP workspace only (members live in their subsystems; no code). |
| [embewi-agent](https://github.com/iobewi/embewi-agent) | Application behavior and its own HTTP endpoints; consumes framework services. |

The first execution target uses Embassy with the existing ESP runtime. `iobewi` is a service framework, not a replacement RTOS kernel. Scheduling and timing constraints remain the responsibility of the runtime and platform integration.

## Design constraints

- Keep service interfaces independent of ESP partition, peripheral, and NVS types.
- Preserve a single physical flash owner when storage and OTA share hardware.
- Expose the administrative API through HTTPS; never fall back to plaintext when the TLS identity is unavailable.
- Stream OTA request bodies into the update backend without buffering a whole image in RAM.
- Separate time-critical application tasks from potentially blocking network and flash operations.
- Keep the one-shot provisioning surface separate from the runtime administrative API.

## First milestones

1. Separate the portable HTTP dispatcher and HTTPS listener contract without ESP dependencies. Initial code is present; integration validation is in progress.
2. Connect `iobewi-ota` to the shared HTTP server; the portable OTA routes remain to be implemented.
3. Connect the existing ESP adapter and migrate `embewi-agent` to framework services.
4. Build the same application against a second platform adapter to validate portability.

No milestone is marked complete until the code and its target build demonstrate it.
