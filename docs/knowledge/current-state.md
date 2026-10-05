# Current state

Baseline: S20.

## Stable / validated

- Portable/platform separation across IOBEWI layers.
- ESP32-S3 Agent platform.
- Agent OTA A/B with OTM1 and bootloader authority.
- Workload OTA A/B with OTM2 and WorkloadSupervisor authority.
- Native target-specific Workload execution using IWNI v1.
- Explicit C-compatible Workload ABI and safe `iobewi-workload` Rust SDK.
- Offline restart of a Valid native Workload.
- Native Workload health/quarantine and S20 RTC boot guard.
- SharedFlash single ownership with multicore auto-park.
- Candidate corruption rejection before execution.
- NativeRuntime production path; ProbeRuntime test/fault-injection only.

## Not started / not implemented as a complete capability

- General Workload GPIO capability.
- Workload I2C/SPI/I2S capability model.
- General Workload networking/storage APIs.
- MPU/process memory isolation.
- Multi-Workload execution.
- General preemptive Workload scheduler model.
- Native Workload runtime on RP2350, Teensy 4.1 or ESP32-C3.

## Platform state

- ESP32-S3: native runtime and Workload A/B hardware validated.
- ESP32-C3: several platform/storage/build components exist; native Workload execution is
  not qualified as the S3 runtime is.
- RP2350 / Teensy 4.1: architectural direction only; not implemented.

See `open-debts.md` for known debt.
