# ADR-0008 — IWNI v1 native Workload image

Status: Accepted.

## Decision
IWNI v1 is the native Workload container. ELF is a deterministic build intermediate,
not the public OTA/runtime image contract.

## Consequences
The loader validates a small explicit format rather than implementing a general dynamic
ELF loader.
