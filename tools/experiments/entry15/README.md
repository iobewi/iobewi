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
The fixture panic handler is explicit; production handler ownership remains to design.

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
| Entry task future | 2064 bytes, alignment 4 |
| Linker main stack reservation | 318564 bytes |

The generic fixture holds a 2048-byte buffer across await. The main-stack reservation
is a linker quantity, not measured free stack; future storage lives in the Embassy
static pool. There is no heap in this fixture. These numbers are not StreamBeWI's
actual future, memory/heap requirements or runtime high-water marks.

## Updated USB decision (2026-10-05)

The user selected a simpler product lifecycle: read otg_enabled from StreamBeWI's
ConfigSpace during boot, before initializing either native USB controller.
Absent/false selects provisioning (Serial/JTAG only); true selects OTG (no JTAG).
Successful provisioning durably sets true, effective only at reboot or replug.
Recovery resets false before clearing credentials and rebooting.

Hot JTAG-to-OTG handover, its arbiter and retirement experiments are removed from
the required entry milestone. The HAL finding (pending JTAG futures lack Drop
interrupt cleanup) remains true but is avoided by exclusive boot construction.
Console auto-backend must not touch JTAG in the OTG boot.

This fixture proves compile/link, descriptor identity, chip metadata and measured
fixture future layouts. It does not yet implement ConfigSpace mode selection,
production Board, resource budget validation or the actual StreamBeWI composition.
Those are subsequent implementation gates. No attached serial device is available
here, so neither physical boot-mode verification nor BG-ESP-S3/BG-USB-MSC is claimed.
Production future/heap/stack measurements remain pending. The compile experiments
can now be reviewed independently of the removed running USB transition.
