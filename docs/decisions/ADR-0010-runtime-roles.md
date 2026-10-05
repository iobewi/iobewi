# ADR-0010 — NativeRuntime production, ProbeRuntime tests

Status: Accepted.

## Decision
NativeRuntime is the production runtime. ProbeRuntime is retained for test/fault
injection only; feature selection prevents ambiguous coexistence.

## Consequences
Production execution has one unambiguous runtime path while lifecycle fault tests retain
a cheap backend.
