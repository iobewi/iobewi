# Native Workload image — IWNI v1

A Workload is a **native binary compiled for one target**. The OTA stores a file; this is the
container format of that file and the gate that decides, *before any byte is copied to
executable memory or any jump is made*, whether it may run. Crate: `iobewi-workload-image`
(`no_std`, no platform). Packer: `iobewi-workload-pack`.

* It is **not ELF.** An ELF is only the intermediate build artefact; the packer reads the
  allocatable sections inside the target's code/data regions and nothing else.
* It is **not a Rust ABI.** Only little-endian integers; the entry point and the context are
  `extern "C"`/`repr(C)` (see `workload-runtime-api.md`).
* It does **not repeat OTM2.** Id, version, digest and size are OTM2's; the SHA-256 of the whole
  file is the digest OTM2 verifies (at upload, and again before every start).
* **No relocation.** The image is linked for the target's fixed region; the header states the
  addresses and the loader refuses any other.

## Header (64 bytes, little-endian)

| offset | size | field | meaning |
|---:|---:|---|---|
| 0 | 4 | `magic` | `"IWNI"` (not `OTM1`/`OTM2`/`S18PROBE`) |
| 4 | 2 | `format_version` | `1`. Container layout. |
| 6 | 2 | `header_len` | `64` |
| 8 | 2 | `target` | `1` = ESP32-S3 (Xtensa LX7), `2` = ESP32-C3 (RV32IMC, representable, no loader yet). Never reused. |
| 10 | 2 | `abi_version` | Version of `WorkloadContext` the entry point expects (`1`). |
| 12 | 2 | `requires.major` | `RuntimeApi` the Workload needs |
| 14 | 2 | `requires.minor` | |
| 16 | 4 | `flags` | none defined in v1: any set bit is refused |
| 20 | 4 | `image_size` | whole file; must equal the size OTM2 recorded |
| 24 | 4 | `entry` | link address of `workload_entry` (inside the code area, 4-aligned) |
| 28 | 4 | `code_addr` | link address of the code area (= the target's fixed code base) |
| 32 | 4 | `code_offset` | file offset of the code bytes (≥ 64) |
| 36 | 4 | `code_size` | code bytes (including literals); > 0 |
| 40 | 4 | `data_addr` | link address of the data area (= the target's fixed data base) |
| 44 | 4 | `data_offset` | file offset of the initialised data (rodata + data); `0` if none |
| 48 | 4 | `data_size` | initialised bytes |
| 52 | 4 | `bss_size` | zero-initialised bytes following the data |
| 56 | 8 | reserved | must be zero |

File layout: `header | code | rodata+data`. Code first, data after, no overlap, inside `image_size`.

## Three versions, three meanings

| number | says | bumped when |
|---|---|---|
| `format_version` | how this *container* is laid out | the header/segments change incompatibly |
| `abi_version` | how the `WorkloadContext` handed to the entry point is laid out | the context layout changes incompatibly (additive growth uses the `size` fields) |
| `RuntimeApi` (major.minor) | what the Agent *functionally* offers (global Agent ↔ Workload compatibility, also in OTM2) | services are added (minor) or removed/changed (major) |

`RuntimeApi 1.0` = `log` + `time` + `control`, with `abi_version 1`. The Workload requires an API;
the Agent provides one; `provided.major == required.major && provided.minor >= required.minor`.

## The gate (`ImageHeader::validate`) — order and reasons

Evaluated by `preflight` at **activation time** (candidate stays `Staged` on refusal, HTTP
`422 {"error":"image_rejected","reason":…}`) and again by `start` (boot, rollback restore).
All arithmetic is checked (`checked_add`); nothing is read past `artifact.size`.

| check | reason |
|---|---|
| artifact shorter than a header | `truncated` |
| magic | `bad_magic` |
| `format_version != 1` | `bad_format_version` |
| `header_len != 64` | `bad_header_len` |
| reserved bytes / flags | `reserved_not_zero` / `unknown_flags` |
| `target` unknown / not this device | `unknown_target` / `target_mismatch` |
| `abi_version` ≠ device's | `abi_mismatch` |
| `RuntimeApi` not satisfied | `runtime_api_mismatch` |
| `image_size` ≠ OTM2 size | `size_mismatch` |
| header API ≠ API declared at `prepare` | `api_declared_mismatch` |
| code empty | `empty_code` |
| offsets/sizes overflow, overlap, outside the image | `bad_bounds` |
| `code_addr`/`data_addr` ≠ the loader's | `addr_mismatch` |
| code or data+bss over the region budget | `too_large` |
| entry outside the code, or unaligned | `bad_entry` |

## ESP32-S3 fixed layout (`TargetLayout`, single source in `iobewi-workload-image`)

| area | link address | capacity | accessed through |
|---|---|---:|---|
| code | `0x403D3700` | 20 KiB (`0x5000`) | instruction bus (executed) |
| data + bss | `0x3FCE8700` | 12 KiB (`0x3000`) | data bus |

The 32 KiB region is the tail of the reclaimed `dram2` area (`0x3FCE3700..0x3FCEB700` on the data
bus). SRAM1 is mapped twice (instruction bus = data bus + `0x6F0000`), so the loader *writes*
the code through the data-bus alias and the core *fetches* it through the instruction-bus alias.
`workload/sdk/ld/esp32s3-v1.ld` repeats these numbers; a host test keeps them equal.

## Build chain

```text
source (Rust, no_std, iobewi-workload SDK)
  -> cargo build (xtensa-esp32s3-none-elf, build-std core, esp32s3-v1.ld)   intermediate ELF
  -> iobewi-workload-pack: ELF -> IWNI, validated with the loader's own gate
  -> SHA-256 of the file (the digest OTM2 verifies)
  -> HTTPS upload through /v1alpha1/workload/ota/{prepare,write}
```

`examples/workloads/build.sh <out>` builds every example image deterministically (path remapping,
no debug info, one codegen unit; `examples/workloads/check-repro.sh` builds twice from a clean target and compares).
Sizes of the example Workload: header 64 B, code 2 056 B, rodata+data 44 B, bss 4 B, image 2 164 B.

Hostile images for the negative tests come from `iobewi-workload-pack --force-…` (bad magic,
format, target, ABI, entry, and `--api 9.9`).
