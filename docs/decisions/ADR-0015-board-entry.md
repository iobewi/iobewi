# ADR-0015 — Board capabilities and framework-owned product entry

Status: Proposed — requires explicit approval before implementation.
Tracking: https://github.com/iobewi/iobewi/issues/15

## Context and evidence

IOBEWI base: `bf0b443` (main after PR #14).
StreamBeWI reference: `feat/iobewi-esp-drivers`, issue reference `9869f4f`.
The inspected target constructs HAL peripherals, allocator, RTOS, UART0,
USB-Serial-JTAG, GPIO0 and an OTG factory. It also hardcodes the NVS range.
The portable app accepts a product-owned Platform struct with two serial ports.

The existing ESP platform crate contains pure memory geometry. The runtime crate
contains diagnostics, not platform startup. Neither is an entry implementation.
The existing WifiTransport keeps NetworkHandle opaque; StreamBeWI currently
requires an Embassy Stack explicitly. Board must not silently generalize that
product dependency or modify the Workload ABI.

## Relationship to ADR-0014

This proposal **partially supersedes ADR-0014** upon acceptance and delivery of
its corresponding implementation: framework-owned capability contracts replace
product-owned ports for Board capabilities, and framework startup/entry replaces
HAL types and concrete wiring in the product target package. Portable business
policy, source/API portability and the explicit Workload ABI remain unchanged.
ADR-0014 remains the available transitional path until those capabilities ship.

At model acceptance, update docs/product-integration.md to distinguish the approved
Board direction from the still-available target-local baseline. Replace its current
ports/target-package instructions only as the new APIs become available; never
present a proposed entry macro as implemented. Update ARCHITECTURE.md at delivery.

## Proposed decision

Introduce a portable `iobewi-board` capability contract and a target-selected
`iobewi-entry` facade. A product's binary contains only:

```rust
#![no_std]
#![no_main]
iobewi_entry::entry!(streambewi::run);
```

The product also declares Cargo dependencies/features, its target build configuration
and `build.rs` calling `iobewi_entry_build::emit()`. “No target code” means no HAL
initialization, pin construction, chip-specific entry attribute or linker policy
implemented by the product; it does not mean no target selection/build metadata.

Board is a same-binary, statically dispatched composition contract. It is not a
native Workload capability and never crosses the Agent/Workload binary boundary.
It owns resources by value and splits once into independent product capabilities.
No global peripheral registry, runtime target detection or second flash owner.

## Proposed contract sketch

This is reviewable API design, not an implemented/public API. The names below are
planned exports of the future portable crate. External traits reuse existing
IOBEWI and embedded ecosystem contracts.

```rust
pub struct Serial<R, W> {
    pub rx: R,
    pub tx: W,
}

// A bank exposes 0..n ports without a fixed pair or HAL types in the product.
// One associated RX/TX representation per board; an adapter enum can represent
// heterogeneous hardware without allocation or trait objects.
pub trait SerialBank {
    type Rx: embedded_io_async::Read + 'static;
    type Tx: embedded_io_async::Write + 'static;

    fn take_next(&mut self) -> Option<Serial<Self::Rx, Self::Tx>>;
}

// Consuming the factory guarantees one acquisition per boot.
// Awaiting permits safe retirement of a shared serial peripheral.
pub trait UsbFactory {
    type Driver: embassy_usb::driver::Driver<'static>;
    type Error: core::fmt::Debug;

    async fn acquire(self) -> Result<Self::Driver, Self::Error>;
}

pub trait Reset {
    fn reset(self) -> !;
}

pub struct BoardParts<W, C, S, B, U, R, I> {
    pub wifi: W,
    pub config: C,
    pub serial: S,
    pub button: B,
    pub usb: U,
    pub reset: R,
    pub identity: I,
}

pub trait Board: Sized {
    type Wifi: iobewi_wifi_core::WifiTransport;
    type Config: iobewi_config_space::ConfigBackend;
    type Serial: SerialBank;
    type Button: embedded_hal_async::digital::Wait;
    type Usb: UsbFactory;
    type Reset: Reset;
    type Identity: iobewi_device::DeviceIdentity
        + iobewi_device::DeviceMetadata;

    fn into_parts(self) -> BoardParts<
        Self::Wifi, Self::Config, Self::Serial, Self::Button,
        Self::Usb, Self::Reset, Self::Identity,
    >;
}
```

The app signature is `async fn run<B: Board>(board: B)`; each product adds the
bounds it needs (for example `B::Wifi: WifiTransport<NetworkHandle =
embassy_net::Stack<'static>>`, Display for Address and Debug for Config::Error).
Board itself keeps the transport handle opaque and does not impose Embassy net.

SerialBank returns None after its finite configured port list is exhausted.
A product may enumerate it at startup and service the supplied ports with portable
helpers. StreamBeWI's current fixed A/B routing must be adapted; simply wrapping
its existing Platform is not sufficient for the 0..n contract. The first driver
provides UART0 and USB-Serial-JTAG. A fake bank with zero and three ports verifies
that the portable API is not tied to that initial pair.

For a board without a button or USB, future adapters can expose explicit unavailable
implementations; absent input waits must stay pending without spinning, and USB
acquisition must report unsupported capability. These adapters are not evidence
that C3 or RP2350 is supported.

## Shared USB lifecycle

UART0 remains available when OTG starts. On S3, USB-Serial-JTAG and OTG use
GPIO19/20; independent ownership of those pins must never be exposed to the app.

The ESP serial adapter and USB factory share a private arbiter. acquire first
retires JTAG IO: stop admitting operations, wake pending reads/writes with a
documented retired-port error, wait for driver access to end, release/reroute the
shared PHY, then construct OTG. New operations on retired handles fail without
touching hardware. Cancellation of acquire leaves shared serial retired and must
not allow a second acquisition. Failure is explicit; no live JTAG/OTG overlap.

The product decides *when* to call acquire (configured/stream-ready for StreamBeWI).
It treats a retired provisioning port as finished instead of retrying it forever.
This transition needs a dedicated implementation experiment and hardware proof;
the current late FnOnce closure does not establish those guarantees.

USB buffers are framework-owned static resources sized by validated resource requests.
The product retains USB identity, MSC policy and descriptor/class buffer choices.

## Console ownership during USB handover

The current console_print directly calls esp-println. The new S3 composition must
not rely on an automatic console backend that can select USB-Serial-JTAG: that
would bypass the PHY arbiter after retirement. Logs and Improv must share the
same serial ownership rules.

For the first implementation, choose **discard of the physical console output**
from the beginning of JTAG retirement through OTG operation. Atomically disable
admission of console writes and finish any in-flight access before changing the
PHY. Console writes must be bounded even before retirement: a disconnected or
backpressured host must never block startup or USB handover. The console callback
must not await an async lock, perform an unbounded flush or spin waiting for a host.
After retirement it returns immediately without touching JTAG. Portable ring
capture and configured network log consumers remain independent of this physical
sink. A later UART fallback would require arbitration with Improv on UART0 and
is not assumed here.

The handover experiment must exercise logs with a connected host, a disconnected
host and a stalled host, including concurrent serial IO. Explicitly measure the
bound; an unbounded esp-println backend cannot satisfy this contract unchanged.

## Entry, startup and profiles

- Put pure board profile data in `arch/esp32/platform`: button pin/polarity,
  UART pins, NVS label and physical memory limits. Keep that crate host-testable.
- Put HAL startup in a platform module of `arch/esp32/runtime`: clock, allocator,
  timer/RTOS, one-time resource initialization and Board construction.
- Put OTG device glue in `drivers/usb/esp32`, serial transports in a dedicated
  `drivers/serial/esp32`, and input glue in `drivers/input/esp32`.
- Keep existing device metadata/identity, Wi-Fi and config backend implementations.
- Discover NVS by label/type/subtype using partitions-esp32 and validate bounds,
  alignment and capacity against physical flash. Missing/invalid NVS is an explicit
  startup error; never fall back to 0x9000/0x6000 or erase on discovery failure.
- Reuse the process-wide SharedFlash, preserving multicore_auto_park where needed.
- Reset is an owned capability; its ESP implementation selects the appropriate
  physical reset scope, including USB link cleanup, without moving recovery policy
  into the framework.
- Socket count is a **product request**, not a board fact: StreamBeWI requests
  three slots for HTTP, DNS and DHCP. The product declares portable resource data
  (socket count and required buffers/heap); entry passes it to the platform before
  Board construction. The platform validates it against the board's memory limits
  and reserves the matching static resources. Impossible requests fail clearly;
  no silent count reduction. Const resource declarations may select concrete
  platform generics without exposing HAL types or chip cfg in the app. The exact
  declaration syntax is subject to the milestone-1 experiment. A board profile
  is distinct from a chip feature; boards
  with different wiring can select a framework profile without editing app code.

Exactly one target feature is required. Initially only esp32s3 is implemented;
zero/multiple/unsupported target features and feature/--target mismatches fail
clearly. Adding another supported target changes the framework entry adapter and
the consumer's target-selection metadata, not its business source or entry line.

The external macro emits a concrete non-generic entry function and awaits the
monomorphized product future. It must not emit a generic Embassy task.
Hidden facade exports may provide implementation dependencies using $crate;
procedural attribute paths and downstream expansion must be tested with the real
ESP toolchain. Do not assume transitive dependency names are available.

The image descriptor must be emitted at the downstream expansion site, with an
explicit experiment proving that its package name/version are the product's.
The build helper runs in the downstream build script because dependency linker
arguments do not propagate to the product binary.

Startup errors are reported by framework diagnostics and terminate startup without
starting the app or erasing storage. Panic/backtrace handlers belong to the entry
composition and must not be duplicated by product dependencies.

## Alternatives considered

- Product-owned Platform/targets: retains the duplication reported in #15.
- A universal Board with raw peripherals: leaks HAL and pin ownership into products.
- Fixed serial_a/serial_b: repeats one product's assumptions rather than 0..n.
- Immediate OTG factory plus independent JTAG ports: cannot express safe ownership
  transfer of the shared PHY.
- Forcing Embassy Stack on Board: needlessly narrows the existing Wi-Fi contract.
- Routing Board through the native Workload ABI: violates INV-010 and changes scope.

## Invariants and consequences

Preserve INV-001, INV-004, INV-005, INV-008, INV-010 and INV-020.
Existing OTA formats, slot authority and independently built Workload ABI are unchanged.
ARCHITECTURE.md currently says the product supplies the PHY driver: update that
statement only when the accepted design is implemented, not in this proposal.

New framework crates need canonical READMEs and generated AGENTS files.
The portable app may depend on framework capability contracts and portable services,
but not esp-hal, esp-rtos, physical pins, ESP adapters or chip-name literals.

## Device identity for Improv

DeviceIdentity supplies the hardware identifier/MAC, not the chip name.
DeviceMetadata::chip_name() returns &'static str; the current ESP implementation
uses esp_metadata_generated::chip_pretty!(). The app passes
identity.chip_name().as_bytes() to Improv instead of a product ESP literal.
EspDeviceIdentity and EspDeviceMetadata are currently separate types: the ESP
Board identity adapter must delegate both traits rather than claiming either
existing type already satisfies the combined bound. Milestone 1 must verify
that the S3 metadata yields exactly b"ESP32-S3" in the selected locked/latest
builds; inspecting the macro invocation alone does not prove its expanded value.

## Approval levels

1. Board model: consuming parts, finite serial bank and asynchronous consuming
   USB acquisition are the architectural model approved in principle by the user,
   subject to this review's corrections. No entry implementation is thereby proven.
2. entry!: **conditionally proposed**, accepted only after milestone 1 proves an
   external macro can emit the concrete ESP entry, preserve the product descriptor
   and fit the measured future in the available memory. If any proof fails, revise
   the entry design before proceeding, keeping the approved Board model.

## Milestones after model approval

1. Compile experiments for external entry expansion, product descriptor identity,
   concrete main future size and JTAG-to-OTG retirement. Record actual measurements.
2. Portable Board contract with fake capabilities and serial-bank tests.
3. ESP drivers, profile, single-owner startup and entry/build helper.
4. Migrate StreamBeWI to Board; remove targets/esp32; retain one entry binary.
5. Local locked/latest builds, documentation generation/check, then CI and hardware.

Each milestone remains reviewable. Stop before merge or the next milestone unless
authorized, following root AGENTS.md.

## Acceptance evidence

- A minimal downstream fixture with no direct HAL/RTOS dependencies compiles via
  entry! and the build helper; image descriptor matches fixture name/version.
- Record the product main future's actual storage size and linker RAM/heap/stack
  headroom; do not use a guessed universal threshold.
- Test bounded console discard/retirement for connected, absent and stalled hosts.
- Test zero/three serial ports, pending IO retirement and no busy retry loop;
  acquisition cannot access shared pins while JTAG IO is active.
- Test NVS discovery failure and validate one SharedFlash owner.
- Replay BG-ESP-S3 and BG-USB-MSC on hardware; host tests/builds alone are partial.
- Run esp locked/latest CI and docs_tool.py generate/check.
- StreamBeWI has no target-specific source and retains provisioning, BOOT recovery,
  streaming and MSC policy in its portable product code.

## Review outcome and next gate

The user approves the Board model in principle. This revision addresses numbering,
partial supersession, physical log ownership, product-declared socket resources
and conditional entry approval. Milestone 1 is the next validation gate.
Do not merge or claim entry! accepted until the relevant approval/proofs exist.
