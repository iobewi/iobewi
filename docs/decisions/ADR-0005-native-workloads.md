# ADR-0005 — Native target-specific Workloads

Status: Accepted.

## Decision
Workloads are separately built native binaries for their target architecture. Portability
is source/API portability through IOBEWI, not binary portability.

## Consequences
The runtime can remain close to hardware and compatible with future RT/capability work,
while each target receives its own binary.
