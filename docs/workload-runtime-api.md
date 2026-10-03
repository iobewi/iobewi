# Workload runtime API and ABI (v1)

What a native Workload may rely on, and what the Agent guarantees. Crates:
`iobewi-workload-abi` (the binary contract), `iobewi-workload` (the safe Rust API over it),
`iobewi-workload-native` (the runtime policy), `iobewi-esp-workload::native` (ESP32-S3 backend).

```text
workload application (Rust, no_std)
        |  safe API: Context, Logger, Time, Control            iobewi-workload
        v
binary contract: WorkloadContextV1 + service tables, repr(C)    iobewi-workload-abi
        v
Agent-provided services (log ring, monotonic clock, control block)

Agent side:  WorkloadSupervisor -> NativeRuntime -> image gate/loader -> native entry
```

## No Rust ABI crosses the boundary

Forbidden between the Agent and a Workload binary: `dyn Trait`, references, `Box`, `String`,
`Vec`, Rust enums, `Future`, Rust-ABI functions, Embassy types. Allowed: `repr(C)` structs of
fixed-size integers and `AtomicU32`, explicit pointers (32-bit on every defined target), `extern
"C"` function pointers with integer/pointer arguments, `i32` status codes. Agent and Workload
may be built with different toolchains.

## Entry point

```c
int32_t workload_entry(const WorkloadContextV1 *ctx);   // extern "C", 4-aligned
```
Returns `0` for a clean stop. The Workload discovers services only through `ctx`; there are no
global Agent symbols, no dynamic linking, no magic addresses. `ctx` is valid for the whole
execution and must not be modified or freed.

## `WorkloadContextV1` (32-bit pointers, 40 bytes, align 4)

| offset | size | field | |
|---:|---:|---|---|
| 0 | 4 | `size` | `sizeof(WorkloadContextV1)`, lets a newer Workload detect an older Agent |
| 4 | 4 | `abi_version` | `1` |
| 8 | 2 | `runtime_api_major` | what the Agent provides |
| 10 | 2 | `runtime_api_minor` | |
| 12 | 4 | `flags` | reserved, 0 |
| 16 | 4 | `control` | `*const ControlBlockV1` |
| 20 | 8 | `log` | `LogServiceV1 { u32 size; write }` |
| 28 | 12 | `time` | `TimeServiceV1 { u32 size; monotonic_us; sleep_us }` |

Offsets are asserted at compile time on 32-bit targets and by host tests (`iobewi-workload-abi`).

## `ControlBlockV1` (32 bytes, align 4, all `AtomicU32`)

| offset | field | written by | meaning |
|---:|---|---|---|
| 0 | `size` | Agent | 32 |
| 4 | `abi_version` | Agent | 1 |
| 8 | `state` | both | `1` STARTING (Agent) → `2` RUNNING (Workload, on entry) → `3` STOPPED (entry returned) / `4` FAILED (panic) |
| 12 | `stop_requested` | Agent | non-zero: please return from the entry point |
| 16 | `progress` | Workload | proof-of-life counter; health = it advances |
| 20 | `exit_code` | Workload | the entry point's return value |
| 24 | `fault` | Workload | `0` none, `1` panic |
| 28 | `reserved` | | |

## Services (calling convention: C ABI, integer/pointer arguments, `i32` status)

| service | signature | cost (ESP32-S3 backend) |
|---|---|---|
| `log.write` | `i32 (u32 level, const u8 *msg, u32 len)` | copy ≤ 120 B into a 2 KiB SPSC ring (never blocks, drops when full); the Agent forwards **2 lines per 100 ms** (20/s) to its synchronous logger and reports drops once a second. The limit exists because the logger blocks on the serial port: unthrottled forwarding starved the Agent's executor on hardware |
| `time.monotonic_us` | `i32 (u64 *out)` | one SYSTIMER read |
| `time.sleep_us` | `i32 (u32 us)` | busy-wait on the Workload core; returns `STOP_REQUESTED` as soon as a stop is requested |
| control | direct atomic access to `ControlBlockV1` | one atomic op |

Status codes: `0` OK, `-1` INVALID, `-2` STOP_REQUESTED, `-3` UNSUPPORTED. Levels: 1 error, 2 warn,
3 info, 4 debug. Time is a monotonic microsecond counter since boot, independent of NTP. Strings
are `pointer + length`, UTF-8, not NUL-terminated; the Rust façade takes `&str`.
No service allocates; the contract needs no `alloc`.

## Lifecycle and supervision

```text
activate : preflight (gate)  -> persist Activating -> stop old -> start new -> PendingConfirmation
start    : gate -> halt previous -> clear region -> copy code/data -> control = STARTING
           -> launch on the second core -> wait for RUNNING (1 s) -> Ok
health   : RUNNING and `progress` moved within 3 s => Healthy; FAILED/STOPPED/frozen => Unhealthy
stop     : stop_requested=1 -> wait up to 1.5 s for the entry point to return (cooperative)
           -> otherwise the Workload core is parked (forced, "StopTimeout" is logged) -> no
              Workload instruction runs afterwards
running  : the identity (digest) of what was loaded, only while STARTING/RUNNING
```
The stop is **cooperative first, forced second**; a Workload that ignores it cannot keep running,
but it gets no cleanup. A panic (SDK `panic-handler`) sets `fault=1`, `state=FAILED` and parks
itself; the Agent halts the core on its next sample and the Supervisor sees `Unhealthy`/`NotRunning`
(confirmation refused, rollback possible).

## Failure semantics (S20)

*Invariant: Agent bootability > Workload availability.* A Workload failure never rewrites OTM2 and
never needs physical access to recover from.

| failure | what happens | Agent | Workload state |
|---|---|---|---|
| panic (SDK handler) | `FAILED`, runtime halts its core on the next sample | up | `Unhealthy`, not running |
| entry point returns | `STOPPED`, core halted | up | `Unhealthy`, not running |
| hardware fault on the Workload core (e.g. jump to 0) | the core stops, `state` stays `RUNNING`, `progress` freezes → `Unhealthy` after 3 s, **quarantined** (core halted) after 10 s more | up (measured) | `Unhealthy`, then not running |
| wedged loop | same as above | up | `Unhealthy`, then quarantined |
| ignores a stop request | forced halt after 1.5 s, `StopTimeout` logged | up | stopped |
| takes the whole chip down (reset, corrupted Agent memory) | **crash-loop guard**, below | boots every time | auto-start suppressed after 3 unclean starts |

`confirm` is refused for any of these (`workload_not_running` / `workload_unhealthy`); a running
Workload is only ever called `Valid` after it was really healthy. Recovery is an ordinary
`upload → activate → confirm` of a replacement: stopping a quarantined Workload is a no-op
(`AlreadyHalted`, not a `StopTimeout`).

### Crash-loop guard (boot)

Only a *whole-chip* reset can loop: a fault on the Workload core does not reset the Agent (measured
on hardware). The guard counts unclean auto-starts in RTC fast RAM (`iobewi-workload-native::boot_guard`):

1. deliberate reboots (`/reboot`, Agent OTA) set a "clean" flag first → the next boot starts with a
   fresh counter;
2. every other boot increments the counter before auto-starting the Workload;
3. after **3** consecutive unclean starts the Workload is **not auto-started**: the Agent boots,
   serves HTTP, the Workload stays `Valid` in OTM2 but does not run (log: `auto-start SUPPRESSED`);
4. a Workload that stays Healthy for **30 s** clears the counter.

The state survives resets, not a power cycle (a power cycle forgets and gives one more try). It is
**not** in OTM2 (formats unchanged). Limit: it protects availability of the Agent, not the Workload
(a defective Workload stays suppressed until replaced, or the device is power-cycled/rebooted on
purpose).

## Runtime roles

| runtime | role | selected by |
|---|---|---|
| `NativeRuntime` | **the** production runtime: native IWNI images | default feature `workload-native` |
| `ProbeRuntime` | test backend only: lifecycle/fault tests with `S18PROBE` artefacts (a validation artefact, not a Workload format) | `--no-default-features --features test-probe-runtime` |
| none | an Agent with no Workload runtime (activate/confirm/rollback answer 501) | `--no-default-features` |

`workload-native` and `test-probe-runtime` cannot coexist (explicit `compile_error!`;
`scripts/check-feature-matrix.sh` checks every combination). The route
`POST /v1alpha1/workload/ota/test/corrupt-candidate` (post-staging corruption for hardware tests)
exists only in an image built with `test-fault-injection`; a production build answers 404.

## Ownership (ESP32-S3)

| thing | owner | placement |
|---|---|---|
| code, rodata, data, bss | the Workload image, loaded by the Agent | the 32 KiB Workload region (fixed address) |
| stack | the Agent allocates it for the Workload core | 8 KiB static |
| `WorkloadContextV1`, service tables | Agent, immutable while running | Agent static |
| `ControlBlockV1` | Agent memory, shared atomics | Agent static |
| log ring | Agent memory, producer = Workload core | Agent static, 2 KiB |
| heap | none: the Workload has no allocator | |

Budget of the POC: 20 KiB code, 12 KiB data+bss, 8 KiB stack. The Workload cannot use "all the RAM
of the Agent" *by contract*; see Isolation for what is not enforced.

## Isolation — what is and is not protected

**Protected:** integrity of the stored image (SHA-256 re-checked before every start); a wrong
target/ABI/API/format/bounds/entry never reaches executable memory; the Agent's executor is never
blocked (the Workload runs on the other core); a Workload can always be stopped (cooperative, then
parked); flash writes park the Workload core so it never fetches from a switched-off flash cache.

**Not protected:** this is the **trusted native Workload model**. Nothing (no MPU/PMS configuration)
prevents a Workload from reading or writing any Agent memory, peripheral or flash register. The
SHA-256 guarantees integrity, **not harmlessness**. A hardware fault on the Workload core (illegal
instruction, jump to 0, bad pointer) raises an exception handled by the Agent's `esp-hal` handler:
it **prints the panic and backtrace and stops that core; it does not reset the chip**. Measured on
hardware with the `fault-null-jump` image (`InstrProhibited` on AppCpu): the Agent kept serving
HTTP, the Workload became `Unhealthy` once its progress counter stopped (3 s window), `confirm` was
refused (409 `workload_unhealthy`) and `rollback` stopped the dead core and reloaded the previous
Workload on it. This is *fault containment by core*, not memory isolation: a Workload that corrupts
Agent memory can still take the Agent down. Do not call this process isolation. A future step may add memory-protection capabilities.
Signatures are not part of S19.

## Evolution

Add services as new `*ServiceV1`-style tables and grow `WorkloadContextV1` (bump the `size`; a
Workload checks `ctx.size`); raise `RuntimeApi.minor`. Hardware capabilities (GPIO, I²C, SPI, I²S,
network) will be new handles in the context, possibly backed by a fast dedicated path rather than a
call into the Agent; none exist in S19. Preemption, a dedicated core and hard-real-time IRQs are not
precluded by this contract and not implemented.
