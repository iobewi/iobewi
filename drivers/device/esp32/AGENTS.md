# Agent Context — iobewi-esp-device

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-device`
- Path: `drivers/device/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

Two distinct ESP providers implementing portable device identity and metadata.

## Owns

- Read the eFuse base MAC and delegate identifier formatting to `iobewi-device`.
- Report compile-selected chip name and DRAM address-range size.

## Does not own

No Wi-Fi startup, station-interface MAC selection, heap/PSRAM probing, product
name prefix, partition-layout metadata or complete Board profile.

## Architecture position

Platform adapter at `drivers/device/esp32` implementing
[portable device contracts](../core/README.md) through esp-hal and generated metadata.

## Public contracts

- `EspDeviceIdentity` implements `DeviceIdentity`: `hardware_id()` reads
  `esp_hal::efuse::base_mac_address()` and returns the last three bytes as
  lowercase hex via `hardware_id_from_mac`; `mac_address()` returns `Some`
  containing all six base-MAC bytes.
- `EspDeviceMetadata` implements `DeviceMetadata`: `chip_name()` uses
  `esp_metadata_generated::chip_pretty!()`; `ram_size()` is the byte difference
  between the end and start of `memory_range!("DRAM")`.

Both types are zero-sized, `Clone + Copy + Default`; identity does not implement
metadata and metadata does not implement identity. Import the corresponding portable
trait to call its methods. Select one matching `esp32s3` or `esp32c3` feature.

## Invariants

Repository-wide invariants apply; platform dependencies remain in this adapter.

## Modification context

### Lifecycle

Construct the unit providers or use `Default`; no initialization, HAL peripheral
handle or exclusive ownership token is taken. Identity methods read eFuse on each
call; metadata is compile-selected, not detected at runtime. The returned hardware
ID allocates a `String`, so the firmware must initialize its allocator first.
No method returns `Result`; allocation failure follows the allocator's behavior.

## Required validation

Cross-check `iobewi-esp-device --features esp32s3` on Xtensa (Rust CI).
For behavioral changes run `BG-ESP-S3`, compare base MAC/ID to eFuse and metadata
to the selected chip's generated DRAM range. C3 is an exposed feature, not proof
of the S3 native Workload qualification on C3.

## Known limitations

The base MAC need not equal every network-interface MAC. Its 24-bit suffix is
not globally unique. `ram_size()` counts the generated DRAM range, including
reserved/used space; it is neither free RAM nor total internal/external memory.
No resource arbitration or hardware startup is provided.

## Related components

- [Portable identity/metadata](../core/README.md)
- [ESP runtime measurements](../../../arch/esp32/runtime/README.md)
- [Product integration](../../../docs/product-integration.md)

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
