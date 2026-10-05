# ADR-0014 — Portable product policy and target composition roots

Status: accepted. Recorded: 2026-10-05.

## Context

Issue #13 exposes a documentation gap: products could read the framework rules
without knowing where to assemble ESP adapters. This records the per-target
composition decision referenced by the issue as taken on 2026-10-01; it does not
change the implemented APIs or the native Workload execution model.

## Decision

A product keeps its business state, policy and generic orchestration in a portable
no_std crate consuming IOBEWI ports. Each target has a separate composition package
that initializes hardware, owns concrete resources, binds adapters and spawns
concrete Embassy task wrappers around generic product futures. iobewi-esp-* and
HAL/radio types are confined to that target package, including target-local board
and task modules. Missing framework capabilities use narrow product-owned ports
with target-local implementations until a separate framework change is justified.

## Consequences

Source/API portability follows INV-001 and INV-008; each target still builds its
own binary. The composition root contains wiring rather than business policy.
The product remains responsible for dependencies/features, single shared flash
ownership, applicable contracts and hardware validation. Within one image generic
Rust interfaces are allowed; the independently loaded Workload boundary still
requires the explicit C-compatible ABI (INV-010).

See [the integration guide](../product-integration.md) for the structure, current
ports/adapters, external Cargo consumption and review checklist.
