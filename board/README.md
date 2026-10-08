---
layer: portable-contract
status: implemented
invariants:
  - INV-001
  - INV-004
  - INV-005
  - INV-008
  - INV-010
  - INV-020
gates: []
---

# iobewi-board

## Summary

Portable `no_std` contracts for a board consumed once into independent product
capabilities and a native USB mode selected once at boot. The contract is
implemented; production board adapters and framework entry are not supplied yet.

## Responsibilities

- Define owned Board/BoardParts, finite SerialBank, boot I/O selection and Reset.
- Reuse Wi-Fi, configuration, device identity, async serial and button contracts.
- Keep the Wi-Fi network handle opaque and allow product-specific extra bounds.
- Specify exclusive native USB construction without a running handover.

## Non-responsibilities

No HAL initialization, GPIO/pin mapping, allocator, clock, NVS discovery, physical
flash construction, socket reservation, console/panic handler or entry macro.
No persisted flag schema, provisioning/recovery policy, serial response routing,
MSC readiness, USB protocol implementation or automatic restart.

## Architecture

This is a same-binary portable composition contract, as recorded in
[ADR-0015](../docs/decisions/ADR-0015-board-entry.md). Platform startup constructs a
Board; the product consumes its capabilities. Board is not a native Workload
capability and does not cross the independent Agent/Workload ABI.

The USB bound uses `embassy-usb-driver::Driver<'static>` directly. It is the same
trait reexported by `embassy_usb::driver`, with no dependency on the USB stack,
executor, time runtime or platform HAL. Existing configuration and identity
contracts use `alloc`; applications still need an allocator for those operations.
The Board contract itself allocates nothing and requires neither Send nor Sync.

## Public API

| API | Contract |
| --- | --- |
| `Board::into_parts(self)` | Transfer Wi-Fi, config backend, button, unselected boot I/O factory, reset and combined identity/metadata by value |
| `BoardParts<W, C, B, IO, R, I>` | Independent capability fields; no global registry or raw peripherals |
| `Serial<R, W>` | Separate owned receive/write halves for one port |
| `SerialBank::take_next(&mut self)` | Transfer 0..n configured ports, then return None permanently |
| `UsbBootMode` | Provisioning or MassStorage, selected by the product before native USB initialization |
| `BootIoFactory::select(self, mode)` | Synchronous consuming construction; return an explicit error for unsupported mode or failed construction |
| `BootIo<S, D>` | Mode-specific serial bank and optional USB driver; Provisioning returns None, MassStorage returns Some(driver) |
| `Reset::reset(self) -> !` | Owned platform reset; product owns its timing and persistence policy |

Board keeps `WifiTransport::NetworkHandle` opaque. A product can add an equality
bound to the network handle its services need, without imposing that bound on all
boards. Config backend errors and address formatting similarly remain the product's
additional bounds. Identity implements both DeviceIdentity and DeviceMetadata;
`chip_name()` supplies the name reported to Improv, while `hardware_id()` and the
optional MAC remain distinct capabilities.

SerialBank exposes no names/count or fixed pair. A consumer repeatedly takes ports
until None, then services the transferred halves independently. Implementations
may use enums for heterogeneous hardware, without a trait object or allocation.
Physical port selection and ordering belong to the board profile; portable
consumers must not infer UART/JTAG identity from an index.

### Product resource declaration

ResourceRequest declares sockets, ap_sockets, heap_bytes and minimum_stack_bytes before platform construction. `sockets` sizes the station's network stack; `ap_sockets` sizes the soft access point's own stack and is 0 for a product that has no access point (at least 2 otherwise: the platform's address service takes one). A product that uses the access point adds the bound `B::Wifi: WifiAccessPoint` to its `run<B: Board>`. The product chooses socket counts; the platform admits the byte requirement or fails, without reducing it. The minimum stack is a linker reservation requirement, not a measured free-stack guarantee.

## Lifecycle

1. Platform startup constructs Board, leaving both native USB controllers uninitialized.
2. The product consumes BoardParts and loads its configuration before calling select.
3. Provisioning constructs Serial/JTAG and no OTG. MassStorage constructs OTG and
   neither initializes nor returns Serial/JTAG. Independent UARTs may remain.
4. The product starts its services with the selected resources. Persisting a new
   mode changes only a later boot; there is no second selection or live handover.

The consuming APIs protect one Rust value. Implementations must not duplicate
unique hardware owners through Clone/Copy or a second constructor; the type system
does not certify an adapter's physical behavior. ConfigBackend's existing Clone
bound permits shared access handles, never an independent second flash owner.
Factories must clean up failed construction and never leave both USB controllers
active. Console and panic paths must also respect this ownership.

Unsupported capabilities are explicit. A future absent-button implementation must
wait pending without spinning; an unsupported USB mode returns an error. This
crate defines no production unavailable adapters. Public BootIo fields make the
mode-dependent Option and serial contents semantic implementation obligations,
not an automatically enforced hardware check.

Product-specific ConfigSpace flag handling and persistence failures follow the
ADR. The contract does not load, repair or erase configuration itself. Product
resource requests, including sockets, are validated by future platform startup;
this milestone adds no resource-declaration syntax.

## Invariants

- INV-001 / INV-008: no platform dependency, HAL type or chip pin in the contract;
  same business source may be composed for multiple boards.
- INV-004 / INV-005 / INV-020: adapters preserve one physical SharedFlash owner and
  discover/validate partitions; this contract creates no flash or partition.
- INV-010: no Rust trait/future/reference crosses the independently built binary ABI.

## Validation

Run `cargo test -p iobewi-board --locked`: eight host integration tests exercise
zero/three ports, permanent exhaustion, independent RX/TX ownership, deferred
USB selection, exclusive fake constructors, unsupported OTG, opaque non-Embassy
network handles, config/identity consumption and pending button input. Two
compile-fail doctests reject reusing a consumed Board or boot I/O factory.

The fakes live only under tests/support. Their USB token counts constructor
selection and cannot start a USB stack; the tests are contract/consumer evidence,
not physical USB qualification or proof that every possible adapter is compliant.

Run workspace checks with all/no-default features, `cargo tree -p iobewi-board`
(no esp-* platform dependencies), documentation generate/check and MkDocs strict.
A focused Xtensa no_std cross-check confirms target compilation. Existing CI also
runs the portable tests; no hardware gate is claimed at this milestone.

## Known limitations

No production Board, ESP serial/input/OTG adapters, startup or build helper.
No runtime budget, actual StreamBeWI future/heap/stack measurement, NVS validation,
multicore parking, physical reset/USB behavior, fatal-error signaling or
console/panic feature audit is proven. Those remain later ADR milestones;
BG-ESP-S3 and BG-USB-MSC require hardware. The entry15 experiment retains its own
reduced mock contract and is not a production integration of this crate.

## Related components

- [Product integration](../docs/product-integration.md) and [ADR-0015](../docs/decisions/ADR-0015-board-entry.md).
- [Wi-Fi transport](../net/wifi/core/README.md), [configuration](../fs/config/README.md).
- [Device identity/metadata](../drivers/device/core/README.md), [USB MSC](../drivers/usb/msc/README.md).
- [Repository invariants](../INVARIANTS.md), [validation gates](../docs/validation/baseline-gates.md).
