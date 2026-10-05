# Agent Context — iobewi-esp-runtime

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-runtime`
- Path: `arch/esp32/runtime`
- Layer: `platform-architecture`
- Status: `implemented`

## Role

ESP stack painting and heap measurements implementing portable `RuntimeDiagnostics`.
This crate does not start the HAL, RTOS or Embassy executor.

## Owns

- Paint unused main-stack memory once and scan the surviving prefix for headroom.
- Report currently free bytes in the global `esp_alloc::HEAP`.

## Does not own

No allocator setup, task spawning, scheduler, overflow prevention, per-task stack
accounting or second-core/Workload-stack diagnostics.

## Architecture position

The platform implementation at `arch/esp32/runtime` consumes the portable
[diagnostics contract](../../../drivers/diagnostics/core/README.md).
It uses architecture assembly and linker symbols, not a portable host implementation.

## Public contracts

- `EspRuntimeDiagnostics::initialize() -> Self` paints `[ _stack_end, current SP )`
  with `0xAA` and returns a zero-sized, `Clone + Copy` handle.
- With one of `esp32s3`/`esp32c3`, the handle implements `RuntimeDiagnostics`:
  `stack_headroom_bytes()` scans from `_stack_end` toward `_stack_start` until
  the first non-`0xAA` byte; `heap_free_bytes()` returns `esp_alloc::HEAP.free()`
  converted to `u32`. The latter is a current free-byte count, not a high-water
  mark or largest contiguous allocation.

## Invariants

Repository-wide invariants apply; this crate declares no additional invariant.

## Modification context

### Lifecycle

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

## Required validation

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

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
