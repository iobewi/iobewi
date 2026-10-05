---
layer: portable-contract
status: implemented
invariants:
  - INV-001
  - INV-002
  - INV-003
  - INV-009
  - INV-011
  - INV-021
gates: []
---

# iobewi-update-model

## Summary

Portable vocabulary and independent in-memory Agent and Workload OTA activation policies.

## Responsibilities

Model artifact compatibility, staging, activation and confirmation through separate boot and Workload authorities, including RuntimeApi compatibility checks.

## Non-responsibilities

Physical flash writes, durable metadata, actual reboot, native loading and atomic Agent+Workload releases.

## Architecture

Portable policy model shared by update domains. It models decisions; production persistence/execution are in update/platform layers.

## Public API

`AgentOta`, `WorkloadOta`, `InstalledAgent`, `InstalledWorkload`, `Activation`, `Confirmation`, `Reason` `RuntimeApi`, `ArtifactDescriptor`, `UpdateRequest`, `AbSlots`, `BootAuthority` and `WorkloadSupervisor`. Agent activation uses BootAuthority and requires reboot; Workload activation uses WorkloadSupervisor and returns Switched.

## Invariants

- `INV-001`
- `INV-002`
- `INV-003`
- `INV-009`
- `INV-011`
- `INV-021`

## Validation

`cargo test -p iobewi-update-model` verifies independent update policies and compatibility refusal. See docs/dual-ota.md for the canonical model.

## Known limitations

State here is in memory, not a restart-safe transaction store. Agent confirmation checks compatibility with the active Workload; Workload activation checks the running Agent API before supervisor effects. No physical-slot selection by the remote control plane.

## Related components

`firmware/update`, `workload/update`, `docs/dual-ota.md`.
