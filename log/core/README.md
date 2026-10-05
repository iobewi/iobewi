---
layer: portable-contract
status: implemented
invariants:
  - INV-001
gates: []
---

# iobewi-log

## Summary

Local, process-wide log capture without a network dependency.

## Responsibilities

Install the global logger and capture formatted messages in a critical-section protected FIFO. Invoke the caller-supplied console callback before attempting ring capture.

## Non-responsibilities

Console hardware, log delivery, persistence and network retry policy.

## Architecture

Portable capture layer consumed by log/stream; the target supplies the console callback and application target prefix.

## Public API

`install(print, application_target)` is called once during single-threaded startup. `pop_line()` removes the oldest message; `discard()` clears the ring. `Line`, `LINE_MAX` (160 bytes), `RING_CAPACITY` (24 lines) and `LogMetadata` define capture and delivery metadata.

## Invariants

- `INV-001`

## Validation

`cargo test -p iobewi-log` covers filtering, overflow, oversized messages and discard.

## Known limitations

The global maximum level is Info. Targets starting with `application_target` or `iobewi_log` accept Info and above; all other targets accept Warn and above. Debug/Trace are excluded. Oversized messages and new messages arriving at a full ring are dropped, not truncated or used to evict old messages. Captured text contains record arguments only, not original level/target. Installation is not repeatable.

## Related components

`log/stream` consumes this FIFO; `drivers/console/esp32` provides the ESP callback.
