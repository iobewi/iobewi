# Repository invariants

These identifiers are stable. Crate README files reference them instead of copying the
full rule.

## INV-001 — Portable/platform dependency direction

Portable code MUST NOT depend on platform implementations such as `esp-hal`.
Platform crates MAY depend on portable contracts.

## INV-002 — Control plane does not select physical slots

Core or another control-plane client MUST NOT select physical Agent or Workload A/B
slots. It selects logical targets/artifacts; local boot/runtime policy selects slots.

## INV-003 — Agent OTA and Workload OTA are independent

Agent OTA and Workload OTA MUST remain separate transactions and persistent lifecycles.
They MUST NOT be turned into an atomic Agent+Workload release pair.

## INV-004 — One ESP physical flash owner

There MUST be exactly one process-wide physical ESP flash owner. Code MUST NOT create an
independent second `FlashStorage` instance.

## INV-005 — SharedFlash is the shared flash path

ESP subsystems sharing physical flash MUST use `SharedFlash`. The native S3 path MUST
preserve `multicore_auto_park` while a Workload may execute on the second core.

## INV-006 — Workloads are native target-specific binaries

The selected Workload execution model is native target-specific code.

## INV-007 — Wasm/wasmi are not the selected runtime

Wasm, wasmi, WASI and an interpreted business VM are outside the selected Workload
architecture unless a future ADR explicitly replaces this decision.

## INV-008 — Portability is source/API portability

The same business source may target multiple platforms through IOBEWI contracts. The
same binary is not expected to run across different architectures.

## INV-009 — OTM1 and OTM2 have distinct ownership

OTM1 belongs to Agent OTA. OTM2 belongs to Workload OTA. Their formats and lifecycles
MUST NOT be conflated.

## INV-010 — No Rust ABI across the Agent/Workload boundary

The binary boundary MUST use explicit C-compatible layouts and calling conventions.
Rust trait objects, Rust references, opaque Futures and unstable Rust layouts MUST NOT
cross it.

## INV-011 — RuntimeApi compatibility gates activation

A Workload may be staged independently of compatibility, but activation MUST reject an
incompatible required RuntimeApi. Agent confirmation MUST also preserve compatibility
with the active Workload.

## INV-012 — Valid Workload supports offline restore

A Valid Workload MUST be restorable by the Agent without requiring Wi-Fi or Core.

## INV-013 — Agent health and Workload health are distinct

A Workload becoming Unhealthy/Stopped MUST NOT by itself make the Agent health endpoint
unhealthy while the Agent remains operational.

## INV-014 — A faulty Workload must not make the Agent unrecoverable

The Agent MUST remain recoverable after a faulty native Workload. The S20 RTC boot guard
suppresses Workload auto-start after three unclean boots and clears after sustained
healthy execution.

## INV-015 — Native Workload is trusted, not isolated

SHA-256/image validation provides integrity, not memory safety. The current architecture
does not claim MPU/process isolation between Agent and native Workload.

## INV-016 — NativeRuntime is production; ProbeRuntime is test-only

`NativeRuntime` is the production Workload runtime. `ProbeRuntime` is retained only
for tests/fault injection and MUST NOT silently become a production alternative.

## INV-017 — Persistent state follows real lifecycle transitions

Persistent OTA state MUST be advanced only with the corresponding real lifecycle action
and MUST reconcile safely after reset/power loss.

## INV-018 — Supersession invalidates staged metadata before overwrite

When a new Workload upload starts overwriting the inactive slot, any prior Staged
candidate referring to those bytes MUST be invalidated before the first overwrite.

## INV-019 — Candidate integrity is checked before native execution

A staged native Workload candidate MUST have its integrity/structure revalidated before
code is loaded or executed. A corrupted candidate MUST NOT be jumped to.

## INV-020 — Partition capability is discovered, not invented

Platform code MUST discover named partitions and validate bounds. It MUST NOT infer
missing Workload storage from unused-looking flash.

## INV-021 — Boot authority differs by update target

The bootloader owns physical Agent boot selection. The WorkloadSupervisor/NativeRuntime
path owns physical Workload activation. The bootloader MUST NOT choose Workload slots.
