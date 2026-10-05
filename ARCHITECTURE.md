# IOBEWI architecture

IOBEWI is a portable, `no_std` embedded framework. Platform-independent contracts and
services live above platform adapters and hardware drivers. The current validated
platform is ESP32-S3; ESP32-C3 is represented in several lower layers but does not yet
carry the native Workload runtime validated on S3.

## System model

```text
portable contracts / services
            |
            +-----------------------------+
            |                             |
            v                             v
   firmware update model          Workload model / SDK
            |                             |
            v                             v
   firmware update runtime        WorkloadSupervisor
                                          |
                                          v
                                     NativeRuntime
                                          |
                                          v
                                   native Workload
            |                             |
            +--------------+--------------+
                           |
                           v
                  platform adapters
                           |
                           v
                        drivers
                           |
                           v
                       hardware
```

IOBEWI separates reusable policy from platform execution. Portable crates define the
contracts, lifecycle models and services. Platform crates bind those abstractions to a
specific target and its HAL.

A product may compose these building blocks into a resident supervisor and may connect
that supervisor to an external control plane, but those product-level components are
consumers of IOBEWI rather than part of the framework architecture.

## Portability boundary

Portable crates define contracts, policy and reusable services. Hardware-specific crates
implement those contracts. Workload business code consumes the Workload SDK/capabilities
and must not depend directly on `esp-hal` or another platform HAL.

Portability is source/API portability. Native Workload binaries are target-specific.

For external products, follow [the integration guide](docs/product-integration.md)
and [ADR-0014](docs/decisions/ADR-0014-product-composition.md): portable product
policy consumes ports; a target composition package owns HAL resources, concrete
adapters and task wrappers. The guide lists the current ESP bindings and gaps.

## Update domains

IOBEWI defines two independent update domains:

- **Resident firmware OTA** uses OTM1 and physical firmware A/B slots. Platform boot
  authority selects the resident firmware slot at boot.
- **Workload OTA** uses OTM2 and physical Workload A/B slots. The Workload supervisor
  owns Workload activation, health confirmation and rollback.

A remote control plane may select a logical artifact/target, but physical A/B selection
remains local to the corresponding IOBEWI lifecycle.

Resident firmware and Workload are not an atomic release pair.

## Native Workload execution

A Workload is a target-specific native Rust binary packaged as IWNI v1. ELF is a build
intermediate, not the public Workload image contract.

The runtime/Workload boundary uses an explicit C-compatible ABI (`repr(C)`, fixed-size
fields and `extern "C"` calls); the Rust ABI never crosses the independently built
binary boundary.

On the validated ESP32-S3 implementation, native Workload code executes on the second
core from a fixed executable RAM region. The current model is trusted native code:
integrity is checked, but there is no MPU/process memory isolation.

## Storage

ESP physical flash has a single owner and is shared through `SharedFlash`. Native
multicore execution uses `multicore_auto_park` so flash operations can safely pause the
Workload core when required.

Partition discovery is by named partition plus bounds validation. Missing capability is
reported rather than inferred from apparently free flash.

## Streaming and virtual media

`stream/rolling` provides a bounded byte window with one rebased consumer session.
`fs/block` defines read-only sector access; `fs/fat16` generates a virtual FAT16
volume over a file source. Their crate README files define the local contracts.
The portable USB MSC class at `drivers/usb/msc` consumes block devices through
Embassy USB driver contracts. The product supplies the device/PHY driver, protocol
identity and unavailable-read policy.
These portable components do not own networking, physical flash or product
buffering policy. They do not add networking/storage capabilities to the native
Workload ABI; a product composes them within its own firmware binary.

## Recovery

Persistent update state is restart-safe. The Workload supervisor reconciles OTM2 after a
reset. A Valid Workload can start offline.

A Workload fault is distinct from resident-system health. The S20 boot guard prevents
repeated unclean boots from making the resident system unrecoverable.

See `INVARIANTS.md` for normative rules and `docs/decisions/` for rationale.
