# Agent Context — iobewi-update-model

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-update-model`
- Path: `firmware/model`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Portable vocabulary and independent in-memory Agent and Workload OTA activation policies.

## Owns

Model artifact compatibility, staging, activation and confirmation through separate boot and Workload authorities, including RuntimeApi compatibility checks.

## Does not own

Physical flash writes, durable metadata, actual reboot, native loading and atomic Agent+Workload releases.

## Architecture position

Portable policy model shared by update domains. It models decisions; production persistence/execution are in update/platform layers.

## Public contracts

`AgentOta`, `WorkloadOta`, `InstalledAgent`, `InstalledWorkload`, `Activation`, `Confirmation`, `Reason` `RuntimeApi`, `ArtifactDescriptor`, `UpdateRequest`, `AbSlots`, `BootAuthority` and `WorkloadSupervisor`. Agent activation uses BootAuthority and requires reboot; Workload activation uses WorkloadSupervisor and returns Switched.

## Invariants

- `INV-001`
- `INV-002`
- `INV-003`
- `INV-009`
- `INV-011`
- `INV-021`

## Modification context

See the canonical README and implementation.

## Required validation

`cargo test -p iobewi-update-model` verifies independent update policies and compatibility refusal. See docs/dual-ota.md for the canonical model.

## Known limitations

State here is in memory, not a restart-safe transaction store. Agent confirmation checks compatibility with the active Workload; Workload activation checks the running Agent API before supervisor effects. No physical-slot selection by the remote control plane.

## Related components

`firmware/update`, `workload/update`, `docs/dual-ota.md`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
