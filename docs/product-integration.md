# Integrating IOBEWI into a product

This guide applies to products such as StreamBeWI that compose IOBEWI services in
one firmware image. The decision is recorded in
[ADR-0014](decisions/ADR-0014-product-composition.md). Preserve INV-001 and INV-008:
business source is portable; the firmware binary and its composition are target-specific.

This is the **current transitional integration path**.
[ADR-0015](decisions/ADR-0015-board-entry.md), tracked by
[issue #15](https://github.com/iobewi/iobewi/issues/15), records the approved Board
direction: framework-owned startup/adapters, consuming capabilities and exclusive
USB selection at boot. The portable [Board contract](../board/README.md) is now
implemented and host-tested, but there is no production Board adapter or framework
entry yet. Keep the target-local composition below until those implementations ship;
the experiments do not constitute an available production entry API. ADR-0015
partially supersedes the corresponding ADR-0014 rules as its implementation ships.

## Read before assembling

Read the repository [AGENTS.md](../AGENTS.md), [architecture](../ARCHITECTURE.md),
[invariants](../INVARIANTS.md), this guide, then the canonical READMEs and generated
AGENTS files for each capability consumed. Read the [contract index](contracts/README.md)
and every applicable contract **before** writing assembly code. In particular,
USB products need the [MSC wire contract](../drivers/usb/msc/README.md) and
[virtual FAT16 contract](../fs/fat16/README.md); products with OTA or independently
loaded Workloads need the corresponding persistent/image/ABI contracts.

## Product repository structure

Use a root workspace for portable code, for example with `members = ["product"]`
and `exclude = ["targets/esp32"]`. Give the target directory its own workspace
(`members = ["firmware"]`) and make `targets/esp32/firmware` depend on
`../../../product` by path. Suggested files:

| Path | Responsibility |
| --- | --- |
| `product/src/lib.rs` | `#![no_std]`, product state, policy and generic service orchestration |
| `product/src/provisioning.rs` | UNCONFIGURED/CONFIGURED transitions and recovery policy |
| `product/src/stream.rs` | Stream selection, prebuffer and unavailable-read policy |
| `targets/esp32/Cargo.toml` | Product's separate ESP workspace and dependency versions |
| `targets/esp32/firmware/src/main.rs` | HAL/runtime startup, one-time resource creation, concrete task spawning |
| `targets/esp32/firmware/src/board.rs` | Pins, peripherals and local adapters for missing capabilities |
| `targets/esp32/firmware/src/tasks.rs` | Concrete Embassy task wrappers calling generic product futures |

`iobewi-esp-*`, `esp_hal`, `esp_radio`, GPIO/peripheral types and concrete stack
handles belong only to the target composition package. A composition root can
span these target-local modules; it need not be one giant main.rs. It creates
resources and wires ports, while product policy stays in the portable crate.
Do not disguise a platform dependency by reexporting its types through product
ports. For example WifiTransport's Address/NetworkHandle associated types are
opaque in the framework contract. Prefer portable I/O contracts in business
policy. ADR-0015 allows products to add the NetworkHandle bounds their current
services require (including an Embassy Stack); Board itself must not impose those
bounds on every product or expose ESP HAL types.

Generic async functions can live in the product crate. An
`#[embassy_executor::task]` function must be concrete, so put a wrapper with
concrete adapter arguments in the target package and await the generic service
there. Keep static buffers, allocator setup, interrupts and peripheral ownership
in that package. Create exactly one physical flash owner and share it through
SharedFlash (INV-004/005), preserving multicore_auto_park when native Workloads run.

## Ports and current ESP bindings

Names below are implemented APIs, not a promise that every platform has an adapter.
Cargo.toml remains authoritative for features and exact dependencies.

| Portable capability | Current ESP binding | Composition responsibility |
| --- | --- | --- |
| `WifiTransport` (`net/wifi/core`) | `WifiManager<SOCKETS>` in `iobewi-esp-wifi` (`drivers/net/wifi/esp32`) | Create radio/runtime/network resources; pass transport to portable Wi-Fi manager |
| `WifiProvisioning` | Portable `iobewi-wifi-manager` over that transport | Product supplies persistence and provisioning workflow |
| `ConfigBackend` (`fs/config`) | `NvsConfigBackend`, `iobewi-esp-config-space` (`fs/nvs/config-esp32`) | Supply SharedFlash, discovered NVS partition and space budgets |
| `ConnectionListener` / `Close` (`net/io`) | `EspTcpListener` / `EspTcpStream`, `iobewi-esp-tcp` | Supply stack, port and buffers; management routes use TLS |
| `TlsDialer` (`net/tls/core`) | `EspTlsDialer`, `iobewi-esp-tls` | Initialize TLS and inject it into portable TLS service with configuration/time |
| `SecureClientTransport` (`net/tls/core`) | `SecureConnector` in portable TLS service composed with EspTlsDialer | Certificate/time policy is separate from low-level dialing |
| Secure server listener | `EspTlsListener`, `iobewi-esp-tls` | Compose TCP listener and server certificate/config provider |
| `EntropySource` (`crypto/rng`) | `EspEntropySource`, `iobewi-esp-entropy` | Supply shared entropy to services |
| `StatusIndicator` / capabilities | `EspStatusIndicator`, `iobewi-esp-ws2812` | Configure pin and spawn led_task; product selects semantic status |
| Logger console callback `fn(&Record)` | `console_print`, `iobewi-esp-console` | Install iobewi-log once with the application target prefix |
| `ArtifactStorage`, OTA runtime ports | `EspArtifactStorage`, shared-flash/service adapters, `iobewi-esp-ota` | Compose transaction state, discovered storage, watchdog and boot authority |
| HTTP OTA `RebootPort` | Product-local wrapper using `iobewi-esp-reset` | Schedule deferred RTC reset; raw reset functions are not a RebootPort implementation |
| Read-only USB MSC driver bound | Embassy USB driver supplied by product | Construct USB OTG device/PHY driver and buffers; IOBEWI supplies MSC protocol |

### Existing ESP capabilities to reuse

Before implementing target-local helpers, consult these canonical contracts:

| Existing capability | Provider and scope |
| --- | --- |
| Boot memory geometry | [ESP platform](../arch/esp32/platform/README.md): C3/S3 `BOOT_MEMORY_MAP`, not board pinout/startup |
| Main-stack and heap diagnostics | [ESP runtime](../arch/esp32/runtime/README.md): initialize once and early on the main stack; does not start HAL/RTOS |
| Hardware identity | [Device contracts](../drivers/device/core/README.md) and [ESP providers](../drivers/device/esp32/README.md): `EspDeviceIdentity` for base MAC/ID |
| Static chip facts | `EspDeviceMetadata` is a separate provider for chip name/DRAM range size; not free RAM |
| Console output | [ESP console](../drivers/console/esp32/README.md): synchronous callback, default auto backend, polling/drop and PHY-sharing limitations |

These APIs are implemented today; they do not supply the proposed Board/entry
startup or exclusive boot-time serial/USB construction. In particular, do not infer a complete MCU
pinout from the boot memory map or HAL initialization from runtime diagnostics.

### Capabilities still supplied locally by the product

- **Plain outbound TCP connector:** `net/io::Connector` exists, but the current
  tree only provides `SecureConnector` through the TLS service. `EspTcpListener`
  is inbound and is not an outbound connector. There is no plain ESP TCP
  Connector adapter yet. A product needing plain HTTP must currently supply
  target-local connection wiring; exposing `NetworkHandle = embassy_net::Stack`
  in portable product signatures is a portability debt, not an exception to
  INV-001/008. Issue #15 should cover an outbound adapter so product HTTP code
  consumes portable async I/O instead of requiring the concrete stack.
- **Button input:** no framework portable button port/ESP adapter currently exists.
  Define a narrow product port or pass sampled semantic events from a target-local
  GPIO adapter. The product owns hold duration, debounce and recovery decisions.
- **Improv Serial transport:** the external [improv-serial](https://github.com/iobewi/improv-serial)
  crate already supplies portable parsing/framing (`no_std + alloc`); its caller
  supplies transport and provisioning actions. IOBEWI does not yet supply the
  concrete UART/Serial-JTAG adapter or task wiring. Keep these target-local and
  coordinate console writes; do not pass the HAL serial type into business logic.
- **USB OTG creation:** the portable MSC class does not create the ESP OTG/PHY
  driver. The target owns it, USB identity and concrete Embassy task wrappers.
- **Reset:** raw software/RTC reset functions exist, but deferred HTTP reboot
  orchestration remains target composition. Never reset before the response can
  leave the device.

These gaps are documented boundaries, not new framework APIs introduced by this guide.

## External Cargo consumption

IOBEWI's portable workspace is at the repository root; platform members declare
`workspace = "../../targets/esp32"` (adjusted by crate depth). A consuming product
has **its own** workspace: do not copy that declaration into product packages or
add product files to IOBEWI's workspace. Git package resolution uses package names;
there is no path suffix to add to a git URL.

Pin every direct IOBEWI dependency to the same full commit SHA and commit the
product Cargo.lock. Example template (replace `IOBEWI_FULL_COMMIT_SHA` in both declarations with
the same full commit containing the WS2812 package rename; update all rev values
together when upgrading):

```toml
# product/Cargo.toml
[dependencies]
iobewi-indicator = { git = "https://github.com/iobewi/iobewi", rev = "IOBEWI_FULL_COMMIT_SHA" }
```

```toml
# targets/esp32/firmware/Cargo.toml
[dependencies]
product = { path = "../../../product" }
iobewi-esp-ws2812 = { git = "https://github.com/iobewi/iobewi", rev = "IOBEWI_FULL_COMMIT_SHA", default-features = false, features = ["esp32s3"] }
```

The current ESP baseline uses esp-hal **~1.2** (1.2.x), and the Wi-Fi driver pins
esp-radio **=1.0.0-beta.1**. See each platform Cargo.toml and the target lockfile
before selecting direct HAL/runtime dependencies; version ranges alone do not
qualify a different radio/TLS combination. Select one chip consistently on every
used ESP adapter (`esp32s3` here); some services require extra opt-in features,
for example `shared-flash` on iobewi-esp-ota. Enabling S3 does not imply C3 feature
parity or native Workload runtime support. Install the appropriate ESP toolchain,
target configuration and linker/build setup for the product firmware separately.

## Minimal policy and binding example

This deliberately small example uses a real portable port and real ESP adapter.
It illustrates the boundary; it is not a complete bootable firmware.

```rust
// product/src/lib.rs
#![no_std]
use iobewi_indicator::{Status, StatusIndicator};

pub fn reflect_stream_state<I: StatusIndicator>(indicator: &I, ready: bool) {
    indicator.set(if ready { Status::Online } else { Status::Connecting });
}
```

```rust
// targets/esp32/firmware/src/board.rs
use iobewi_esp_ws2812::EspStatusIndicator;

pub fn reflect_stream_state(ready: bool) {
    product::reflect_stream_state(&EspStatusIndicator, ready);
}
```

Target startup must additionally spawn `iobewi_esp_ws2812::led_task` with its
RMT peripheral and configured pin for visible output. The portable function can
be tested with a recording StatusIndicator without a HAL. Apply the same pattern
to async services: generic product future, concrete target task wrapper.

## Review and validation before publishing a product change

1. Build/test the portable product crate on the host, with fake ports for policy.
2. Inspect its dependency graph (`cargo tree -p product`): no esp-hal, esp-radio,
   iobewi-esp-* or target adapter dependency, including optional feature paths.
3. Review public signatures and state: no concrete target types leak through ports.
4. Compile the actual target firmware locally before using CI as confirmation.
5. Run applicable [baseline gates](validation/baseline-gates.md) on hardware.
   Host compilation cannot prove USB reader compatibility, watchdog expiry or TLS.
6. Record target, dependency SHA, feature set and hardware evidence in product docs.

Do not confuse a portable crate linked into one firmware with an independently
loaded native Workload. Generic Rust traits/futures can cross the former source
boundary, but must never cross the Agent/Workload binary boundary (INV-010).
Networking/virtual-media composition does not extend the Workload ABI implicitly.
