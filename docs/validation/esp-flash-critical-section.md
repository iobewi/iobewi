# ESP runtime flash interrupt protection

Issue: [#25](https://github.com/iobewi/iobewi/issues/25). Applies to `BG-STORAGE` and `BG-ESP-S3`; this document does not declare either gate passed.

## Scope and source audit

The fix starts from main `96f2197fc558f91aa926209ce0f1cf021dcca3ae`.

| Path | Decision | Reason |
| --- | --- | --- |
| `drivers/flash/esp32/Cargo.toml` | Explicit `esp-storage/critical-section` | The sole runtime owner must protect each ROM operation, independently of consumers. |
| `drivers/flash/partitions-esp32/Cargo.toml` | Explicit same feature | Discovery/raw partition operations and standalone OTA also use this concrete driver; do not rely on another package's feature unification. No owner is constructed here. |
| `firmware/esp32/Cargo.toml` | Unchanged | OTA always depends on partitions; `shared-flash` additionally depends on the owner. Both paths inherit the protected esp-storage instance. |
| `firmware/esp32/src/shared_flash.rs` | Unchanged | Borrowed `guard.storage()` feeds partition erase, artifact writes and EWBT access. Each driver ROM call retains protection, including erase batches. No extra owner or outer section spanning the whole upload is needed. |
| `arch/esp32/boot` | Unchanged, separate boundary | Second-stage boot primitives call ROM directly, outside esp-storage. This fix makes no claim that they gain protection; it addresses runtime NVS/partition/OTA paths. |

In resolved esp-storage 0.10.0, `src/hardware.rs` wraps read, unlock, write and sector/block erase in `maybe_with_critical_section`; `src/lib.rs` selects `esp_sync::RawMutex::lock` only with the feature. esp-sync 0.3.0 `src/raw.rs` raises Xtensa interrupt level to 5 and restores processor state on exit. Higher-level/NMI handlers are not proven safe by this change. The async Embassy mutex guard serializes ownership, but does not hold its bookkeeping critical section throughout an operation.

`src/common.rs` checks whether the other core is running before parking it and resumes it after a write/erase. It does not inspect the existence of an IOBEWI Workload. This policy remains enabled on S3 and is separate from calling-core interrupt masking.

## Reported hardware evidence, not rerun by Codex

Human tests used ESP32-S3 QFN56 rev v0.2, 16 MB flash / 8 MB PSRAM, StreamBeWI `03bef9b8fd50a0564f04e10889e9cc8eeec618f5`, with IOBEWI pinned at `ddfa839a15836c3eebedc24fa34efc1d7bd3bc84` and local patches.

| Image | Observation |
| --- | --- |
| 4A: diagnostic, original features | First 32-byte write at 0x9000 returns; second at 0x9040 does not return. Wi-Fi associated and DHCP obtained. |
| 4B: without auto parking | Same hang; removing parking did not fix this experiment. |
| 4C: with critical-section | All nine credential and six flag writes complete; Improv succeeds. |
| Normal image, only feature fix | Provisioning succeeds without diagnostic traces/physical console. |

Only 4–36 byte writes to virgin NVS were exercised. The interrupt/cache mechanism is a hypothesis. Main at 96f2197 has matching dependency declarations, but was not hardware-tested. Erase/GC, OTA and interrupt latency remain **not tested / not measured**.

## Human hardware procedure

Use an expendable board and record product SHA, IOBEWI fix SHA, Cargo.lock hash, chip/flash identity and toolchain. Preserve any credentials needed before intentionally erasing a test NVS partition. Obtain partition offsets/sizes from the image's partition table; never use the diagnostic addresses above as a general layout.

1. Build StreamBeWI 03bef9b against the fix (local Git-source patches or a reviewed updated pin). Keep the same product boot policy. Record `cargo +esp tree -p streambewi-esp32 --target xtensa-esp32s3-none-elf -e features -i esp-storage` and confirm `critical-section`.
2. Build the release ELF with `cargo +esp build -p streambewi-esp32 --release -Z build-std=core,alloc --target xtensa-esp32s3-none-elf`. Produce a merged image with `espflash save-image --chip esp32s3 --merge --skip-padding <elf> <out.bin>`. Record hashes and flashing settings. A complete installation can erase NVS.
3. Start with virgin test NVS, Wi-Fi active. Provision through Improv; verify both credential and boot-flag commits return and the response is successful. Read back data and reboot. Confirm persisted mode and credentials. Repeat the normal, uninstrumented image test.
4. Perform at least 100 repeated credential/flag commits with alternating values and Wi-Fi traffic. Record return status, elapsed time, read-back and restart persistence. A count alone does not prove erase occurred.
5. Continue until NVS page rotation/GC causes actual sector erase. Capture erase entry/exit and return values in a diagnostic image, then repeat without diagnostics. Require demonstrated erase, correct read-back, successful reboot, and no hang/watchdog reset. If the image cannot exercise GC, mark this step blocked rather than passed.
6. If OTA is part of acceptance, upload a valid image to the inactive slot while Wi-Fi traffic continues. Demonstrate actual erase/program and read-back digest, activation and restart/confirmation. Do not write the active image or unrelated partitions. Record USB/Wi-Fi responsiveness and any resets. Otherwise mark OTA not tested.
7. Measure the protected interval for each ROM read/unlock/write/sector erase/block erase exercised. Instrument the actual driver lock callback with a RAM-safe GPIO pulse or cycle counter; do not format/log while the cache may be disabled. Collect timing after return via UART-only diagnostics or a RAM buffer. Measure wait-for-lock separately if needed. An outer `write`/`erase` measurement is a conservative whole-call duration, not the exact critical-section duration; label it accordingly.
8. Report sample count, flash model, CPU clock, workload, timer resolution, maximum observed interval and separate sector/block erase results. Compare with the product's explicit Wi-Fi/USB/watchdog latency budget. An observed maximum is not a worst-case bound: use the flash datasheet/driver call granularity for a justified bound, or mark the bound **unknown**. Higher-level interrupts/NMI need their own RAM-safety audit if used.

If diagnostics are required, use UART with no `esp-println/auto` feature or mixed sinks; JTAG logging can corrupt Improv. Feature conflicts are build errors, not evidence of flash protection. Keep diagnostic and normal images distinct and archive both.

## Acceptance record

Attach resolved feature graphs before/after, guard negative/positive results, local locked/latest checks and human evidence to the PR. Required hardware results must identify the exact tested fix SHA. A stalled operation, false success, corrupted read-back, unacceptable latency or missing erase evidence prevents a hardware PASS.

| Evidence | Current state |
| --- | --- |
| Small-write provisioning on ddfa839 plus local feature change | Human-reported success |
| Main plus this fix on hardware | Pending |
| NVS repeated writes and actual erase/GC | Pending |
| Maximum critical-section duration / justified worst-case bound | Not measured / unknown |
| OTA under Wi-Fi load | Not tested |

## Resolved graph evidence (local)

Command, in the baseline checkout then the fix checkout:

```sh
cargo +esp tree --manifest-path targets/esp32/Cargo.toml --locked \
  --target xtensa-esp32s3-none-elf -p iobewi-esp-flash \
  --features esp32s3 -e features -i esp-storage
```

Before (96f2197; checkout path normalized):

```text
esp-storage v0.10.0
├── esp-storage feature "embedded-storage"
│   └── iobewi-esp-flash v0.1.0 (<repo>/drivers/flash/esp32)
│       ├── iobewi-esp-flash feature "default" (command-line)
│       └── iobewi-esp-flash feature "esp32s3" (command-line)
├── esp-storage feature "esp-hal"
│   └── esp-storage feature "esp32s3"
│       └── iobewi-esp-flash feature "esp32s3" (command-line)
├── esp-storage feature "esp-rom-sys"
│   └── esp-storage feature "esp32s3" (*)
├── esp-storage feature "esp-sync"
│   └── esp-storage feature "esp32s3" (*)
└── esp-storage feature "esp32s3" (*)
```

After (fix; checkout path normalized):

```text
esp-storage v0.10.0
├── esp-storage feature "critical-section"
│   └── iobewi-esp-flash v0.1.0 (<repo>/drivers/flash/esp32)
│       ├── iobewi-esp-flash feature "default" (command-line)
│       └── iobewi-esp-flash feature "esp32s3" (command-line)
├── esp-storage feature "embedded-storage"
│   └── iobewi-esp-flash v0.1.0 (<repo>/drivers/flash/esp32) (*)
├── esp-storage feature "esp-hal"
│   └── esp-storage feature "esp32s3"
│       └── iobewi-esp-flash feature "esp32s3" (command-line)
├── esp-storage feature "esp-rom-sys"
│   └── esp-storage feature "esp32s3" (*)
├── esp-storage feature "esp-sync"
│   └── esp-storage feature "esp32s3" (*)
└── esp-storage feature "esp32s3" (*)
```

The guard returned exit 1 on the original owner graph with `FAIL: esp-storage must enable critical-section`, then exit 0 for all five consumer graphs after the fix. The negative run used `--toolchain 1.95.0` (Cargo tree only); the positive locked run and graph outputs above used `+esp`. CI uses `+esp`.

## Local verification

Local ESP toolchain: cargo 1.97.0-nightly (c980f4866, 2026-06-30), rustc 1.97.0-nightly (8ea53bcd7, 2026-07-08).

All five ESP checks passed in each mode. Locked uses the committed lockfile and `--locked`. Latest mirrors CI: remove the lockfile in the test checkout, generate a new one within existing manifest ranges, run without `--locked`, then restore the committed lockfile. No dependency version update is committed. Both resolved esp-hal 1.2.2, esp-storage 0.10.0, esp-sync 0.3.0 and esp-bootloader-esp-idf 0.6.0; latest also changed sdio 0.5.1 to 0.5.2.

```sh
# From the repository root; set FLAGS=--locked for locked, FLAGS= for latest.
python3 tools/ci/check_esp_storage_features.py $FLAGS
for package in iobewi-esp-flash iobewi-esp-partitions iobewi-esp-config-space; do
  cargo +esp check --manifest-path targets/esp32/Cargo.toml $FLAGS \
    -Z build-std=core,alloc --target xtensa-esp32s3-none-elf \
    -p "$package" --features esp32s3
done
cargo +esp check --manifest-path targets/esp32/Cargo.toml $FLAGS \
  -Z build-std=core,alloc --target xtensa-esp32s3-none-elf \
  -p iobewi-esp-ota --features esp32s3
cargo +esp check --manifest-path targets/esp32/Cargo.toml $FLAGS \
  -Z build-std=core,alloc --target xtensa-esp32s3-none-elf \
  -p iobewi-esp-ota --features esp32s3,shared-flash

cargo +1.95.0 test --locked -p iobewi-config-space -p iobewi-nvs-core \
  -p iobewi-ota --features iobewi-ota/runtime,iobewi-ota/config-space
python3 tools/docs/docs_tool.py generate
python3 tools/docs/docs_tool.py check
python3 tools/docs/docs_tool.py site
python3 -m mkdocs build --strict -f .generated/docs-site/mkdocs.yml
# actionlint 1.7.12, with optional external shellcheck disabled:
actionlint -shellcheck=
```

Host result: 75 tests passed (ConfigSpace 8, NVS core 5, OTA 62); no failures. Documentation check: 61 crates; strict MkDocs and actionlint passed. ESP checks retain the upstream warning recommending release builds; these are CI-style compile checks, not timing or linked-image proofs. Hardware release images must use `--release`.

Additional locked C3 smoke checks passed for the two existing CI consumers affected by the dependency change:

```sh
cargo +esp check --manifest-path targets/esp32/Cargo.toml --locked \
  -Z build-std=core,alloc --target riscv32imc-unknown-none-elf \
  -p iobewi-esp-config-space --features esp32c3
cargo +esp check --manifest-path targets/esp32/Cargo.toml --locked \
  -Z build-std=core,alloc --target riscv32imc-unknown-none-elf \
  -p iobewi-esp-ota --features esp32c3,shared-flash
```
