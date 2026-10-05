# Agent Context — iobewi-esp-nvs

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-nvs`
- Path: `fs/nvs/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

ESP NVS view over an exclusively borrowed existing flash owner.

## Owns

Bridge esp-nvs NOR operations and ROM CRC to EspFlash without constructing another physical flash instance.

## Does not own

Namespaces, ConfigSpace keys/framing, quotas, migrations, health policy and ownership of the physical flash.

## Architecture position

Platform adapter between esp-nvs and drivers/flash/esp32. The caller acquires SharedFlash access before opening the view.

## Public contracts

`NvsPartition { offset, size }`, `NvsPartition::new`, `NvsFlash` and `open(&mut EspFlash, NvsPartition)` returning a borrowed `Nvs<NvsFlash>`.

## Invariants

- `INV-004`
- `INV-005`

## Modification context

See the canonical README and implementation.

## Required validation

Build through targets/esp32 for the selected chip; BG-STORAGE validates the shared flash path and hardware NVS operations when implementation changes.

## Known limitations

The NVS view must not outlive the exclusive flash borrow. Partition discovery and validation are caller responsibilities; open does not discover partitions. No host execution of the ESP ROM CRC path.

## Related components

`drivers/flash/esp32`, `fs/nvs/core`, `fs/nvs/config-esp32`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
