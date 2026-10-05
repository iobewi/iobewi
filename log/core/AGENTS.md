# Agent Context — iobewi-log

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-log`
- Path: `log/core`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Local, process-wide log capture without a network dependency.

## Owns

Install the global logger and capture formatted messages in a critical-section protected FIFO. Invoke the caller-supplied console callback before attempting ring capture.

## Does not own

Console hardware, log delivery, persistence and network retry policy.

## Architecture position

Portable capture layer consumed by log/stream; the target supplies the console callback and application target prefix.

## Public contracts

`install(print, application_target)` is called once during single-threaded startup. `pop_line()` removes the oldest message; `discard()` clears the ring. `Line`, `LINE_MAX` (160 bytes), `RING_CAPACITY` (24 lines) and `LogMetadata` define capture and delivery metadata.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

`cargo test -p iobewi-log` covers filtering, overflow, oversized messages and discard.

## Known limitations

The global maximum level is Info. Targets starting with `application_target` or `iobewi_log` accept Info and above; all other targets accept Warn and above. Debug/Trace are excluded. Oversized messages and new messages arriving at a full ring are dropped, not truncated or used to evict old messages. Captured text contains record arguments only, not original level/target. Installation is not repeatable.

## Related components

`log/stream` consumes this FIFO; `drivers/console/esp32` provides the ESP callback.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
