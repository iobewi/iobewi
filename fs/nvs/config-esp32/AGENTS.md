# Agent Context — iobewi-esp-config-space

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-config-space`
- Path: `fs/nvs/config-esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

ESP NVS adapter for IOBEWI ConfigSpace

## Owns

- Implement the portable ConfigSpace backend contract using ESP NVS and the shared ESP flash path.
- Use portable NVS record and reservation calculations from `iobewi-nvs-core`.
- Keep that responsibility inside the `platform-adapter` layer.

## Does not own

- Does not create or own an independent physical flash instance.
- Does not define product configuration policy.

## Architecture position

Path: `fs/nvs/config-esp32`. Layer: **platform-adapter**.

Local IOBEWI path dependencies declared by Cargo:
- `../../config`
- `../core`
- `../../../drivers/flash/esp32`
- `../esp32`

## Public contracts

`NvsConfigBackend` implements `ConfigBackend`. Construction receives an existing `&'static SharedFlash` and `NvsPartition`; flash access is serialized through that shared owner. `is_healthy` and `self_check` expose backend health checks. `NvsPartition` and NVS capacity constants are re-exported for composition.

Package features and dependency declarations are canonical in `Cargo.toml`.

### Discovered startup

NvsConfigBackend::from_label(shared_flash, label).await discovers a DATA/NVS partition using the existing partition helper, validates erase geometry against actual flash capacity, releases the shared lock and reuses new(). Errors distinguish discovery, geometry and backend initialization. There is no hardcoded address fallback, new physical flash owner or explicit erase/reformat-on-failure path. Existing esp-nvs open/recovery semantics still apply; initialization is not a guarantee of read-only flash access. Missing partitions/backend failures stop Board startup before any native USB constructor.

## Invariants

- `INV-001`
- `INV-004`
- `INV-005`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-STORAGE`
- `BG-ESP-S3`

## Known limitations

No additional crate-specific limitation is recorded here beyond `docs/knowledge/current-state.md` and `docs/knowledge/open-debts.md`.

## Related components

- Repository `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for machine-readable package facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
