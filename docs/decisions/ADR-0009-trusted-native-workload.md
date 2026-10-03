# ADR-0009 — Trusted native Workload security model

Status: Accepted.

## Decision
The current model treats native Workloads as trusted code. Image SHA/structural checks
provide integrity but no memory sandbox.

## Consequences
Documentation MUST NOT claim process/MPU isolation. Isolation can be added only as an
explicit future architectural capability.
