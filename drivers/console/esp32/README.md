---
layer: platform-adapter
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-console

## Summary

Synchronous ESP console callback delegating a log record to `esp-println`.

## Responsibilities

- Print `LEVEL - message` followed by a newline to the dependency's selected backend.

## Non-responsibilities

No logger installation, filtering, ring capture, streaming, serial receive,
Improv framing, peripheral construction or USB/serial resource arbitration.

## Architecture

Platform sink at `drivers/console/esp32`; target composition passes `console_print`
to the [portable logger](../../../log/core/README.md). This wrapper does not install
esp-println's separate logger even though its dependency enables `log-04`.

## Public API

`console_print(record: &log::Record<'_>) -> ()` exists only with `esp32s3` or
`esp32c3`. It prints the level and formatted arguments, without target, timestamp
or color formatting from esp-println's logger. There is no returned delivery error.

## Lifecycle

Select exactly one chip matching the firmware, then install the application logger
once with this callback. The wrapper takes no UART/USB ownership handle: the
underlying printer accesses hardware directly. Target composition owns the actual
transport configuration and must coordinate any other users of that transport.

The current manifest keeps esp-println's default features. In the locked
`esp-println 0.18.0` these are `auto`, `colors`, `critical-section`; `log-04`
is additionally enabled. On C3/S3, `auto` chooses USB Serial/JTAG if the SOF raw
interrupt flag has been set, otherwise the ROM UART path. The flag is not cleared
by this selection; prior USB connection is not proof of a currently draining host.
IOBEWI exposes chip features only, not backend-selector features. Adding a direct
esp-println dependency with `uart` does not disable the existing `auto` default:
Cargo features are additive and the dependency build script rejects multiple
backend selectors (`auto`, `uart`, `jtag-serial`, `no-op`). Changing that choice
requires a separate manifest/API change.

## Invariants

Repository-wide invariants apply; this sink must not introduce product log policy.

## Validation

Cross-check `iobewi-esp-console --features esp32s3` on Xtensa (Rust CI).
For console/backend changes run `BG-ESP-S3` and test connected, disconnected and
stalled hosts plus the intended serial/OTG composition. Static checks do not prove
physical delivery or real-time timing.

## Known limitations

Output is synchronous and does not yield. The default `critical-section` wraps
formatting/output with esp-sync's `RawMutex`, which disables interrupts on the
current core; this is not a bounded-latency logging API. In 0.18.0 the Serial/JTAG
FIFO wait allows 50,000 polling iterations per wait, then drops remaining bytes;
a remembered timeout avoids another wait while the FIFO remains full. This is
not a millisecond deadline or a whole-record time bound. The UART path calls ROM
transmit/flush functions with no timeout in the Rust wrapper. Delivery/truncation
is not reported to the caller.

On S3, the checked esp-hal 1.2.2 OTG driver selects the internal PHY for OTG
(`usb/otg/ll/esp32s3.rs::fs_common_init` and `usb/otg/mod.rs::common_init`).
That physical path cannot simultaneously serve Serial/JTAG output. esp-println's
SOF-based selection does not arbitrate this handover. Likewise its printer lock
does not serialize an independent HAL UART/Serial-JTAG writer (for example Improv).
Do not assume logs and protocol frames can safely share a writer, or that USB
MSC/OTG preserves the Serial/JTAG console. Board/entry arbitration proposed in
issue #15 is not implemented here.

These dependency details were checked in the versions recorded by the ESP
Cargo.lock; recheck backend/locking/PHY code when upgrading dependencies.

## Related components

- [Portable logger](../../../log/core/README.md)
- [Product integration](../../../docs/product-integration.md)
- [esp-println 0.18.0 source](https://docs.rs/crate/esp-println/0.18.0/source/src/lib.rs)
- [esp-println feature validation](https://docs.rs/crate/esp-println/0.18.0/source/build.rs)
- [esp-hal 1.2.2 OTG source](https://docs.rs/crate/esp-hal/1.2.2/source/src/usb/otg/)
- `Cargo.toml` for actual feature/dependency choices.
