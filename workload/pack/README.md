---
layer: host-tool
status: implemented
invariants:
  - INV-001
  - INV-006
  - INV-007
  - INV-008
  - INV-019
gates:
  - BG-NATIVE-RUNTIME
---

# iobewi-workload-pack

## Summary

Host tool: converts the intermediate ELF of a native Workload into a compact IWNI image (deterministic), validates it with the same gate the loader uses, prints its sizes and SHA-256

## Responsibilities

- Own the Workload-facing contract, policy, build step or validation example described in the summary.
- Preserve the native Workload model and its explicit binary/service boundaries.

## Non-responsibilities

- Does not introduce a Wasm/interpreted runtime.
- Does not make business Workload code depend directly on a platform HAL.
- Does not merge Agent OTA and Workload OTA ownership.

## Architecture

Path: `workload/pack`. Layer: **host-tool**.

Local IOBEWI dependencies declared by Cargo:
- `../image`
- `../abi`
- `../../firmware/model`

## Public API

Exported Rust items (or, for the host tool/example, its documented command/build interface) are the code-level authority. IWNI/ABI/RuntimeApi details remain owned by their canonical contract documents.

## Invariants

- `INV-001`
- `INV-006`
- `INV-007`
- `INV-008`
- `INV-019`

## Validation

- `BG-NATIVE-RUNTIME`

## Known limitations

The current native Workload model is target-specific and trusted; there is no claim of cross-architecture binary portability or MPU/process isolation unless a future contract explicitly adds it.

## Related components

- `docs/native-workload-image.md`
- `docs/workload-runtime-api.md`
- `docs/workload-supervisor.md`
- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
