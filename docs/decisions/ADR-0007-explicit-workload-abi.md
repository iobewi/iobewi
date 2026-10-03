# ADR-0007 — Explicit Agent/Workload binary ABI

Status: Accepted.

## Decision
No Rust ABI crosses the independently built binary boundary. Use fixed C-compatible
layouts/calling conventions; provide a safe Rust SDK above them.

## Consequences
Agent and Workload builds do not rely on unstable Rust layout/trait-object/Future ABI.
