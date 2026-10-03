# Agent Context — iobewi-workload-ota

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-workload-ota`
- Path: `workload/update`
- Layer: `portable-service`
- Status: `implemented`

## Role

Workload OTA: OTM2 persistent record, double-copy metadata store, Workload A/B state machine and recovery (no loader, no runtime, no platform)

## Owns

- Own the Workload-facing contract, policy, build step or validation example described in the summary.
- Preserve the native Workload model and its explicit binary/service boundaries.

## Does not own

- Does not introduce a Wasm/interpreted runtime.
- Does not make business Workload code depend directly on a platform HAL.
- Does not merge Agent OTA and Workload OTA ownership.

## Architecture position

Path: `workload/update`. Layer: **portable-service**.

Local IOBEWI dependencies declared by Cargo:
- `../../firmware/model`
- `../../firmware/update`

## Public contracts

Exported Rust items (or, for the host tool/example, its documented command/build interface) are the code-level authority. IWNI/ABI/RuntimeApi details remain owned by their canonical contract documents.

## Invariants

- `INV-001`
- `INV-006`
- `INV-007`
- `INV-008`
- `INV-002`
- `INV-003`
- `INV-009`
- `INV-011`
- `INV-017`
- `INV-018`
- `INV-019`
- `INV-021`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-WORKLOAD-OTA`
- `BG-NATIVE-RUNTIME`

## Known limitations

The current native Workload model is target-specific and trusted; there is no claim of cross-architecture binary portability or MPU/process isolation unless a future contract explicitly adds it.

## Related components

- `docs/native-workload-image.md`
- `docs/workload-runtime-api.md`
- `docs/workload-supervisor.md`
- Root `ARCHITECTURE.md` and `INVARIANTS.md`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
