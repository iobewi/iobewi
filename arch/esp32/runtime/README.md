---
layer: platform-architecture
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-runtime

## Summary

ESP stack painting and heap measurements implementing portable `RuntimeDiagnostics`.
Diagnostics-only features do not start HAL/RTOS. The optional Board feature adds startup composition; the entry facade owns HAL and executor startup.

## Responsibilities

- Paint unused main-stack memory once and scan the surviving prefix for headroom.
- Report currently free bytes in the global `esp_alloc::HEAP`.

## Non-responsibilities

The diagnostics-only API does not own allocator setup, task spawning or scheduling. No overflow prevention, per-task stack
accounting or second-core/Workload-stack diagnostics.

## Architecture

The platform implementation at `arch/esp32/runtime` consumes the portable
[diagnostics contract](../../../drivers/diagnostics/core/README.md).
It uses architecture assembly and linker symbols, not a portable host implementation.

## Public API

- `EspRuntimeDiagnostics::initialize() -> Self` paints `[ _stack_end, current SP )`
  with `0xAA` and returns a zero-sized, `Clone + Copy` handle.
- With one of `esp32s3`/`esp32c3`, the handle implements `RuntimeDiagnostics`:
  `stack_headroom_bytes()` scans from `_stack_end` toward `_stack_start` until
  the first non-`0xAA` byte; `heap_free_bytes()` returns `esp_alloc::HEAP.free()`
  converted to `u32`. The latter is a current free-byte count, not a high-water
  mark or largest contiguous allocation.

### Optional S3 Board startup

With board-s3-native-usb, platform::Startup<SOCKETS, AP_SOCKETS> prepares the allocator/RTOS and single SharedFlash, then finish(spawner).await discovers NVS by label and builds EspBoard. ResourceRequest `sockets` must match SOCKETS and `ap_sockets` must match AP_SOCKETS (0 builds no access point; 1 is refused at startup, since the address service needs one and nothing would remain); the byte admission counts both stack resource sets; byte admission uses the profile budget and minimum linker stack. Board owns existing Wi-Fi, config, identity, reset and newly supplied UART/button/USB adapters. Its consuming I/O factory constructs JTAG only in Provisioning or OTG only in MassStorage, retaining UART0 in both. The product reads configuration before select; startup does not interpret otg_enabled. StartupFailure::halt emits only a short best-effort UART0 fatal code, with one FIFO-readiness check per byte and no retry/flush. UART constructor failure silently halts. Normal boots and panic have no physical console sink. RTC system reset is used, but PHY behavior after reset still requires hardware. The feature does not start another physical flash owner. Existing diagnostics-only chip features retain their lifecycle. Validate both modes with tools/experiments/board15/run.sh and hardware gates.

## Lifecycle

Call `initialize()` exactly once, as early as possible in the target entry point,
while executing on the stack described by `_stack_end`/`_stack_start` (low/high
addresses, downward-growing stack). Initialize before peripheral setup and task
spawning where possible; any earlier stack history is not reliably captured.
The caller owns stack layout and allocator initialization. Copies share the same
painted memory; they do not start independent measurements. Read diagnostics from
that main-stack context, not an unrelated core/thread stack.

There is no runtime once guard, `Result`, or reset API. If SP is at/below the low
boundary painting returns silently; this is not an overflow detector. Calling
initialization again can erase measurement history.

## Invariants

Repository-wide invariants apply; this crate declares no additional invariant.

## Validation

Run portable `cargo test -p iobewi-runtime`; cross-check `iobewi-esp-runtime`
with `--features esp32s3` on the Xtensa ESP toolchain. `BG-ESP-S3` is required
for changes to painting/layout; verify diagnostics on the actual firmware.

## Known limitations

Assembly is implemented for Xtensa/RISC-V; host compilation is not supported.
The trait implementation requires an ESP chip feature. Linker symbols must match
the live stack. Measurements cover one linker-defined main stack, not separate
RTOS radio-thread or second-core stacks. Painting is a heuristic: used bytes that
match `0xAA` can overestimate headroom. There is no synchronized multicore snapshot.

## Related components

- [Portable runtime diagnostics](../../../drivers/diagnostics/core/README.md)
- [Product integration](../../../docs/product-integration.md)
- `src/lib.rs` and `Cargo.toml` for assembly, features and allocator dependency.
