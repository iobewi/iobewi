# IOBEWI

IOBEWI is a portable Rust `no_std` framework for embedded services, update lifecycle
and native supervised Workloads.

The current validated baseline is **S20**. ESP32-S3 is the primary hardware validation
target. Agent OTA, Workload OTA, native Workload execution, rollback/recovery and the
single-owner shared-flash model have been exercised on hardware.

## Architecture at a glance

```text
embewi-core
    |
    v
embewi-agent
    |
    +-- Agent OTA (OTM1) ------> Agent A/B
    |       bootloader authority
    |
    +-- WorkloadSupervisor
            |
            +-- Workload OTA (OTM2) --> Workload A/B
            |
            v
       NativeRuntime
            |
            v
       native Workload
            |
            v
       iobewi-workload SDK / explicit ABI
            |
            v
       portable services
            |
            v
       platform adapters / drivers
```

Workloads are **native target-specific binaries**. Wasm/wasmi are not the selected
execution model. Portability is at source/API level through IOBEWI contracts.

The Agent is a resident runner/supervisor, not a classical OS kernel. Agent OTA and
Workload OTA are independent lifecycles; Core never chooses physical A/B slots.

## Start here

For humans:

1. [Architecture](ARCHITECTURE.md)
2. [Repository invariants](INVARIANTS.md)
3. [Current state](docs/knowledge/current-state.md)
4. Browse the crate tree: each crate's `README.md` is its canonical local documentation.
5. [Contracts](docs/contracts/README.md) and [decisions](docs/decisions/README.md) for
   normative details and rationale.

For coding agents:

1. Read [AGENTS.md](AGENTS.md).
2. Read the nearest generated crate `AGENTS.md`.
3. Follow the referenced invariants, contracts and validation gates.
4. Never edit a generated crate `AGENTS.md` directly; update its canonical `README.md`.

See [DOCUMENTATION.md](DOCUMENTATION.md) for the single-source documentation model.

## Repository layout

```text
arch/        CPU/SoC/platform architecture primitives
bootloader/  platform boot executables
crypto/      portable crypto contracts and implementations
drivers/     device/network/storage hardware capabilities
firmware/    Agent firmware update/boot/image model
fs/          configuration and persistence layers
log/         local capture and outbound streaming
net/         IO, HTTP, TLS and Wi-Fi contracts/services
time/        time state and NTP synchronization
workload/    Workload ABI, image, OTA, runtime and SDK
targets/     target workspaces/toolchain composition
examples/    validation/example Workloads
docs/        contracts, decisions, validation and knowledge
```

The documentation website is generated from these same Markdown sources. Its crate
navigation mirrors the repository tree rather than maintaining a second hand-written
taxonomy.

## Current constraints

- Portable code must not depend on platform HAL implementations.
- ESP physical flash has one owner and shared users go through `SharedFlash`.
- The Agent/Workload binary boundary never uses the Rust ABI.
- Native Workloads are currently trusted code; SHA/image validation does not provide
  memory isolation.
- A faulty Workload must not make the Agent unrecoverable.
- New architecture decisions and invariant changes require explicit documentation.

## Status and roadmap

See:

- [Current state](docs/knowledge/current-state.md)
- [Open debt](docs/knowledge/open-debts.md)
- [Roadmap](docs/knowledge/roadmap.md)
- [Validation baseline gates](docs/validation/baseline-gates.md)

## License

MIT.
