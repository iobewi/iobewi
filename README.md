# IOBEWI FW

Portable Rust framework for composing embedded services.

> **Status:** architecture and initial documentation. The framework API, platform ports, and migrations described below are planned; this repository does not yet provide a usable firmware crate.

## Goal

`iobewi-fw` defines how embedded applications obtain services through platform-independent contracts. An application such as [embewi-agent](https://github.com/iobewi/embewi-agent) supplies its own behavior and consumes those services. Platform adapters provide the hardware capabilities required to run them on ESP, RP2350, Teensy, or another supported target.

The intended dependency direction is:

```text
embewi-agent (application)
    -> iobewi-fw (service contracts and composition)
        -> services (FiBeWI, httpbewi, ConfigSpace, ...)
        -> platform adapter (espbewi, future RP2350/Teensy adapters)
            -> HAL, network and execution runtime
```

The application should not need an ESP-specific type to use OTA, HTTP, storage, or the watchdog. A service may require capabilities that a given board does not provide; the composition must make that requirement explicit.

## Components and ownership

| Component | Responsibility |
| --- | --- |
| `iobewi-fw` | Portable service contracts, capability requirements, application bootstrap, and service composition. |
| [FiBeWI](https://github.com/iobewi/fibewi) | OTA lifecycle, transactions, validation policy, and the OTA HTTP service. |
| `httpbewi` (planned) | Shared HTTP(S) server: TLS connection lifecycle, routing, and common security policy. Applications and services contribute routes. |
| [ConfigSpace Manager](https://github.com/iobewi/config-space-manager) | Logical persistent configuration spaces independent of a particular NVS backend. |
| [espbewi](https://github.com/iobewi/espbewi) | ESP adapter: physical flash, NVS, network, TLS integration, watchdog, boot and reboot operations. |
| [embewi-agent](https://github.com/iobewi/embewi-agent) | Application behavior and its own HTTP endpoints; consumes framework services. |

The first execution target uses Embassy with the existing ESP runtime. `iobewi-fw` is a service framework, not a replacement RTOS kernel. Scheduling and timing constraints remain the responsibility of the runtime and platform integration.

## Design constraints

- Keep service interfaces independent of ESP partition, peripheral, and NVS types.
- Preserve a single physical flash owner when storage and OTA share hardware.
- Expose the administrative API through HTTPS; never fall back to plaintext when the TLS identity is unavailable.
- Stream OTA request bodies into the update backend without buffering a whole image in RAM.
- Separate time-critical application tasks from potentially blocking network and flash operations.
- Keep the one-shot provisioning surface separate from the runtime administrative API.

## First milestones

1. Define the portable capabilities and composition API without ESP dependencies.
2. Introduce the shared HTTP(S) host and let FiBeWI supply its OTA routes.
3. Connect the existing ESP adapter and migrate `embewi-agent` to framework services.
4. Build the same application against a second platform adapter to validate portability.

No milestone is marked complete until the code and its target build demonstrate it.
