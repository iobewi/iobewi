# ADR-0011 — RuntimeApi compatibility

Status: Accepted.

## Decision
Workloads declare a required RuntimeApi. Staging may precede compatibility, but
activation and Agent confirmation preserve compatibility with the active Workload.

## Consequences
Runtime evolution is explicit and independent from IWNI container/ABI format versions.
