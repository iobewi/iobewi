# ADR-0001 — Agent and Workload model

Status: Accepted.

## Context
The resident embedded component supervises application code but is not a classical OS
kernel with processes/syscalls/MMU semantics.

## Decision
Use **Agent** for the resident runner/supervisor and **Workload** for the supervised
application. Avoid kernel/userspace terminology for this model.

## Consequences
Lifecycle and health can be reasoned about independently without claiming OS isolation.
