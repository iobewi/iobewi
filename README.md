# IOBEWI

IOBEWI is a portable Rust `no_std` framework for embedded systems. It provides
platform-independent contracts and services, native Workload execution/update
infrastructure, and the platform adapters required to bind those abstractions to real
hardware.

The current validated hardware baseline is **ESP32-S3**.

## Architecture at a glance

The dependency direction is deliberate:

```text
application / resident supervisor
            │
            v
      IOBEWI portable API
            │
            v
      services / policies
            │
            v
      platform adapters
            │
            v
          hardware
```

Portable crates define capabilities and policy. Platform crates implement those
capabilities. Application code should not require a platform HAL type to consume an
IOBEWI service.

## Native Workloads

An IOBEWI Workload is a **native Rust binary compiled for its hardware target**.

```text
Workload source
      │
      v
iobewi-workload SDK
      │
      v
target-specific native binary
      │
      v
IWNI image
      │
      v
NativeRuntime
      │
      v
platform Workload backend
      │
      v
hardware
```

The Workload uses the services and capabilities exposed through the IOBEWI Workload
contract instead of depending directly on the platform HAL.

Portability is therefore **source/API portability**: the same Workload source can target
different architectures through the same IOBEWI contracts, while each architecture gets
its own native binary.

The independently built runtime/Workload boundary uses an explicit C-compatible ABI and
fixed-size layouts. The Rust ABI does not cross that boundary.

## Firmware and Workload updates

IOBEWI defines two independent update domains:

- **Agent/resident firmware OTA** uses OTM1 and an A/B firmware lifecycle. Physical boot
  selection belongs to the platform bootloader.
- **Workload OTA** uses OTM2 and its own A/B lifecycle. Physical Workload activation,
  health confirmation and rollback belong to the Workload supervisor/runtime path.

The two lifecycles share reusable transaction concepts but remain independent. A remote
control plane selects a logical artifact, not a physical A/B slot.

## Architectural constraints

- Portable code must not depend on platform HAL implementations.
- Platform adapters implement portable contracts; they do not redefine portable policy.
- ESP physical flash has one process-wide owner and shared consumers use `SharedFlash`.
- The native Workload image is validated before execution.
- A valid Workload can be restored without network or control-plane availability.
- The Workload ABI uses explicit C-compatible contracts across independently built
  binaries.
- Runtime compatibility is checked before activating a Workload.
- A Workload failure must leave the resident system recoverable.

The complete normative list is in [INVARIANTS.md](INVARIANTS.md).

## Documentation and project state

Start with:

1. [Architecture](ARCHITECTURE.md)
2. [Repository invariants](INVARIANTS.md)
3. [Current state](docs/knowledge/current-state.md)
4. [Contracts](docs/contracts/README.md)
5. [Architecture decisions](docs/decisions/README.md)
6. [Validation baseline gates](docs/validation/baseline-gates.md)

For coding agents, [AGENTS.md](AGENTS.md) defines the repository-wide workflow and each
crate provides a generated local `AGENTS.md`.

## Consumers

IOBEWI is intended to be composed by embedded products rather than to contain their
business logic. `embewi-agent` is one current consumer of the framework, but its product
architecture is not the architecture of IOBEWI itself.

## License

MIT.

For product authors: [Integrating IOBEWI into a product](docs/product-integration.md).
