# Integrating IOBEWI into a product

This guide applies to products such as StreamBeWI that compose IOBEWI services in
one firmware image. The decision is recorded in
[ADR-0014](decisions/ADR-0014-product-composition.md). Preserve INV-001 and INV-008:
business source is portable; the firmware binary and its composition are target-specific.

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
opaque in portable policy; do not constrain them to ESP/Embassy concrete types.

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
| `StatusIndicator` / capabilities | `EspStatusIndicator`, `iobewi-esp-indicator` | Configure pin and spawn led_task; product selects semantic status |
| Logger console callback `fn(&Record)` | `console_print`, `iobewi-esp-console` | Install iobewi-log once with the application target prefix |
| `ArtifactStorage`, OTA runtime ports | `EspArtifactStorage`, shared-flash/service adapters, `iobewi-esp-ota` | Compose transaction state, discovered storage, watchdog and boot authority |
| HTTP OTA `RebootPort` | Product-local wrapper using `iobewi-esp-reset` | Schedule deferred RTC reset; raw reset functions are not a RebootPort implementation |
| Read-only USB MSC driver bound | Embassy USB driver supplied by product | Construct USB OTG device/PHY driver and buffers; IOBEWI supplies MSC protocol |

### Capabilities still supplied locally by the product

- **Button input:** no framework portable button port/ESP adapter currently exists.
  Define a narrow product port or pass sampled semantic events from a target-local
  GPIO adapter. The product owns hold duration, debounce and recovery decisions.
- **Improv Serial transport:** protocol/provisioning logic can use portable
  contracts, but the concrete serial peripheral transport and task wiring remain
  target-local. Do not pass the HAL serial type into business state logic.
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
product Cargo.lock. Example using the baseline reviewed for this guide (update
all rev values together when upgrading):

```toml
# product/Cargo.toml
[dependencies]
iobewi-indicator = { git = "https://github.com/iobewi/iobewi", rev = "879045a75f6f47c55994aaa243fde748f19083bf" }
```

```toml
# targets/esp32/firmware/Cargo.toml
[dependencies]
product = { path = "../../../product" }
iobewi-esp-indicator = { git = "https://github.com/iobewi/iobewi", rev = "879045a75f6f47c55994aaa243fde748f19083bf", default-features = false, features = ["esp32s3"] }
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
use iobewi_esp_indicator::EspStatusIndicator;

pub fn reflect_stream_state(ready: bool) {
    product::reflect_stream_state(&EspStatusIndicator, ready);
}
```

Target startup must additionally spawn `iobewi_esp_indicator::led_task` with its
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
