---
layer: validation-tool
status: implemented
invariants: []
gates: []
---

# gpio15-metadata-proof

## Summary

Host proof of the exact ESP adapter GPIO projection.

## Responsibilities

Compile the actual adapter pins.rs against generated S3 metadata and check sparse GPIO lookup and UART mux functions.

## Non-responsibilities

No HAL initialization, physical pin availability certification or copied SoC tables.

## Architecture

A host-only fixture includes the adapter source; esp-metadata-generated supplies the table. The portable device crate remains independent of ESP.

## Public API

One inventory test checks 45 GPIO descriptors, absent GPIO22 and GPIO43/44 UART alternate functions.

## Invariants

Repository-wide invariants apply. Platform resource ownership stays outside portable policy.

## Validation

cargo test -p gpio15-metadata-proof --locked.

## Known limitations

The metadata dependency requires Rust 1.95; this optional fixture is outside default-members. The projection does not expose every upstream electrical, analog or package restriction.

## Related components

See ADR-0015, docs/product-integration.md and the canonical Board and ESP runtime READMEs in this repository. Cargo.toml defines dependency and feature boundaries.
