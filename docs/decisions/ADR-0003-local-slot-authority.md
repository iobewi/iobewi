# ADR-0003 — Physical A/B slot authority is local

Status: Accepted.

## Decision
Core selects logical target/artifact, never a physical A/B slot. The bootloader owns
Agent boot-slot authority; WorkloadSupervisor owns Workload activation authority.

## Consequences
Platform recovery and slot policy remain local and restart-safe.
