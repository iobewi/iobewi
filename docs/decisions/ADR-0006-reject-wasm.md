# ADR-0006 — Wasm/wasmi rejected for Workload execution

Status: Accepted.

## Context
Business Workload logic may require predictable timing and hardware-close execution.

## Decision
Do not use Wasm, wasmi, WASI or another interpreted business VM as the selected Workload
execution model. Use native target-specific binaries.

## Consequences
Future proposals MUST treat Wasm as a replacement decision requiring a new ADR, not an
open default option.
