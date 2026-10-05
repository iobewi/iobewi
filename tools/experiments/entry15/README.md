# Issue #15: entry compile/link experiments

This isolated, unpublished workspace tests entry macro composition before production
Board/entry implementation. Base: d5222cea41c781d76fbe42df4a706dc74e5ff77b,
including the documentation and WS2812 rename fixes. It does not add a supported
framework API or migrate StreamBeWI.

## Reproduce

Load the ESP toolchain environment (typically source ~/export-esp.sh), then:

```sh
cargo +esp build -p entry15-product-proof --release --locked \
  --manifest-path tools/experiments/entry15/Cargo.toml \
  --target xtensa-esp32s3-none-elf -Z build-std=core,alloc -j 2
python3 tools/experiments/entry15/inspect_elf.py \
  tools/experiments/entry15/target/xtensa-esp32s3-none-elf/release/entry15-product-proof
```

Build only the product: the workspace also contains a host-only build helper.
The product calls that helper from build.rs; linker flags do not rely on propagation
from the facade's build script. The dependency is deliberately renamed entry_api.
The fixture panic handler is explicit and performs no console writes; production
handler ownership and dependency-feature audit remain to implement.

The combined replay is `bash tools/experiments/entry15/run.sh locked` (or `latest`
to update within the manifest ranges first). CI runs both modes. Local cargo update
resolved zero changes: the latest compatible set on this run equals the lockfile.

## Negative case

```sh
cargo +esp check -p entry15-product-proof --release --locked --features async-main \
  --manifest-path tools/experiments/entry15/Cargo.toml \
  --target xtensa-esp32s3-none-elf -Z build-std=core,alloc -j 2
```

Expected failure: esp_hal::main's async expansion hardcodes ::embassy_executor and
::esp_rtos; facade reexports imported with use do not populate the extern prelude.
The default variant uses blocking esp_hal::main plus a concrete Embassy task with
its supported executor-path override. The standard executor is kept on main's stack
for the lifetime of the non-returning main. No generic Embassy task is generated.

## Measured locked result

Full release link succeeded with esp-hal 1.2.2, esp-rtos 0.4.0, Embassy executor
0.10.0 and esp-bootloader-esp-idf 0.6.0. The inspector checks actual ELF symbol bytes:

| Item | Result |
| --- | --- |
| Descriptor product | entry15-product-proof |
| Descriptor version | 7.8.9 (facade version is 0.1.0) |
| Generated chip metadata | ESP32-S3 |
| Product future | 2056 bytes, alignment 4 |
| Entry task future | 2060 bytes, alignment 4 |
| Configured internal heap | 98304 bytes |
| Linker main stack reservation | 220164 bytes |

The product now has `async fn run<B: Board>(board: B)`. The facade constructs an
experiment-only owning MockBoard and passes it directly through entry!, using a
concrete Embassy task. Its associated flash capability has a SharedFlashAccess
bound, but is a simulated owning token, not physical SharedFlash. No NVS or Wi-Fi
startup is proven. The generic fixture holds a 2048-byte buffer, that token and a
128-byte Box across await. The facade initializes a 96-KiB internal heap before
launching the task and selects the maximum CPU clock. Those choices exercise
startup composition, not the full StreamBeWI startup or resource budget. The main-stack reservation
is a linker quantity, not measured free stack; future storage lives in the Embassy
static pool. The heap size is a configured reservation, not measured free heap or
runtime allocation evidence. These numbers are not StreamBeWI's
actual future, memory/heap requirements or runtime high-water marks.

## Updated USB decision (2026-10-05)

The user selected a simpler product lifecycle: read otg_enabled from StreamBeWI's
ConfigSpace during boot, before initializing either native USB controller.
Absent/false selects provisioning (Serial/JTAG only); true selects OTG (no JTAG).
Successful provisioning durably sets true, effective only after a restart or board
power cycle (replug only when USB is the board's sole power source). Read failures
are reported and select provisioning without erasing data. Credential and flag
writes are separate; flag-commit failures are reported as Improv errors.
Recovery resets false before clearing credentials and rebooting.

Hot JTAG-to-OTG handover, its arbiter and retirement experiments are removed from
the required entry milestone. The HAL finding (pending JTAG futures lack Drop
interrupt cleanup) remains true but is avoided by exclusive boot construction.
The first production composition must install no physical console sink in either
mode, and its panic/backtrace paths must never touch JTAG in the OTG boot.

This fixture proves compile/link, descriptor identity, chip metadata and measured
fixture future layouts. It does not yet implement ConfigSpace mode selection,
production Board, resource budget validation or the actual StreamBeWI composition.
Those are subsequent implementation gates. No attached serial device is available
here, so neither physical boot-mode verification nor BG-ESP-S3/BG-USB-MSC is claimed.
Production future/heap/stack measurements remain pending. The compile experiments
can now be reviewed independently of the removed running USB transition.

## Macro hygiene and acceptance boundary

The default macro confines implementation imports, functions and measurement statics
to the reserved `__iobewi_entry15` module. ELF probes use the reserved prefix
`__iobewi_entry15_experiment_`; descriptor and entry symbols required by the platform
are still emitted once. The module and prefix are reserved for this single-entry
experiment, not promises of arbitrary symbol collision resistance. The product
imports esp_hal through the facade and defines all three old ENTRY15_* item names
to exercise the collisions reported in review. Its generic run has no HAL bound;
that import exists only in the fixture as a compile-time collision probe.

The negative assertion relies on diagnostic text and can require updates with
compiler/upstream changes. It demonstrates only the missing direct-dependency
extern-prelude paths in this specific async-main variant; it does not establish
that async entry is generally impossible.

This increment qualifies generic dispatch and the macro mechanism. It does not
accept production entry!: acceptance still needs the real StreamBeWI service-join
future, heap/stack budget, exclusive boot-mode behavior and hardware gates.
The complete proposed Board capabilities, finite serial bank, NVS discovery,
physical SharedFlash ownership and zero/three-port tests remain subsequent work.
