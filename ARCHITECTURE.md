# IOBEWI architecture

IOBEWI is a portable, `no_std` embedded framework. Platform-independent contracts and
services live above platform adapters and hardware drivers. The current validated
platform is ESP32-S3; ESP32-C3 is represented in several lower layers but does not yet
carry the native Workload runtime validated on S3.

## System model

```text
embewi-core
    |
    | logical desired state / artifacts
    v
embewi-agent
    |
    +-- Agent OTA --------------------------> Agent A/B
    |      boot authority: bootloader
    |
    +-- WorkloadSupervisor ----------------> Workload OTA / OTM2
              |
              v
         NativeRuntime
              |
              v
       native Workload
              |
              v
        iobewi-workload
        + ABI/service tables
              |
              v
        portable services
              |
              v
        platform drivers
              |
              v
           hardware
```

The Agent is a resident runner/supervisor, not a kernel in the classical OS sense.
A Workload is the application supervised by the Agent.

## Portability boundary

Portable crates define contracts, policy and reusable services. Hardware-specific crates
implement those contracts. Business Workload code consumes the Workload SDK/capabilities
and must not depend directly on `esp-hal` or another platform HAL.

Portability is source/API portability. Native Workload binaries are target-specific.

## Dual OTA

Agent OTA and Workload OTA are independent:

- Agent OTA uses OTM1 and physical Agent A/B slots. The bootloader owns Agent slot
  selection at boot.
- Workload OTA uses OTM2 and physical Workload A/B slots. The Agent's
  `WorkloadSupervisor` owns Workload activation and rollback.
- Core selects a logical artifact/target; it never selects the physical A/B slot.
- Agent and Workload are not an atomic release pair.

## Native Workload execution

The selected execution model is a target-specific native binary packaged as IWNI v1.
ELF is a build intermediate, not the public Workload image contract. The Agent/Workload
boundary uses an explicit C-compatible ABI (`repr(C)`, fixed-size fields and
`extern "C"` calls); the Rust ABI never crosses the binary boundary.

On the validated ESP32-S3 implementation, native Workload code executes on the second
core from a fixed executable RAM region. The model is currently trusted native code:
integrity is checked, but there is no MPU/process memory isolation.

## Storage

ESP physical flash has a single owner and is shared through `SharedFlash`. Native
multicore execution uses `multicore_auto_park` so flash operations can safely pause the
Workload core when required. Partition discovery is by named partition plus bounds
validation; missing capability is reported rather than inferred from apparently free
flash.

## Recovery

Persistent OTA state is restart-safe. The Workload supervisor reconciles OTM2 after
reset. A Valid Workload can start offline. A native Workload fault is distinct from Agent
health, and the S20 boot guard prevents repeated unclean boots from making the Agent
unrecoverable.

See `INVARIANTS.md` for normative rules and `docs/decisions/` for rationale.
