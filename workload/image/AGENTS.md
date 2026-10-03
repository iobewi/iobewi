# Agent Context — iobewi-workload-image

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-workload-image`
- Path: `workload/image`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Native Workload image format (IWNI v1): header encode/decode, target classes and the structural + compatibility gate evaluated before any code is loaded or jumped to (no platform, no loader, no ELF)

## Owns

- Own the Workload-facing contract, policy, build step or validation example described in the summary.
- Preserve the native Workload model and its explicit binary/service boundaries.

## Does not own

- Does not introduce a Wasm/interpreted runtime.
- Does not make business Workload code depend directly on a platform HAL.
- Does not merge Agent OTA and Workload OTA ownership.

## Architecture position

Path: `workload/image`. Layer: **portable-contract**.

Local IOBEWI dependencies declared by Cargo:
- `../../firmware/model`

## Public contracts

Exported Rust items (or, for the host tool/example, its documented command/build interface) are the code-level authority. IWNI/ABI/RuntimeApi details remain owned by their canonical contract documents.

## Invariants

- `INV-001`
- `INV-006`
- `INV-007`
- `INV-008`
- `INV-011`
- `INV-019`

## Modification context

See the canonical README and implementation.

## Required validation

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
