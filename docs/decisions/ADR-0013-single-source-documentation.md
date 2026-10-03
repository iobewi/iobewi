# ADR-0013 — Single-source crate documentation

Status: Accepted.

## Decision
Each crate README is its canonical local semantic documentation. Crate AGENTS files and
the human website are deterministic projections. Cargo.toml remains canonical for package
and dependency facts.

## Consequences
A crate has one editable semantic source, while humans and agents receive views optimized
for their use. CI rejects missing or stale generated agent views.
