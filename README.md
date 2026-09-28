# IOBEWI

IOBEWI is the company behind this portable Rust framework for embedded services. This repository contains the framework; the company name also covers its other projects.

> **Status:** the first component provides a portable HTTP server contract and TLS-only connection dispatcher. Service composition, OTA routes, platform ports beyond ESP, and application migrations remain in progress.

## Goal

`iobewi` defines how embedded applications obtain services through platform-independent contracts. An application such as [embewi-agent](https://github.com/iobewi/embewi-agent) supplies its own behavior and consumes those services. Platform adapters provide the hardware capabilities required to run them on ESP, RP2350, Teensy, or another supported target.

The intended dependency direction is:

```text
embewi-agent (application)
    -> iobewi (service contracts and composition)
        -> services (FiBeWI, httpbewi, ConfigSpace, ...)
        -> platform adapter (iobewi-esp, future RP2350/Teensy adapters)
            -> HAL, network and execution runtime
```

The application should not need an ESP-specific type to use OTA, HTTP, storage, or the watchdog. A service may require capabilities that a given board does not provide; the composition must make that requirement explicit.

## Components and ownership

| Component | Responsibility |
| --- | --- |
| `iobewi` | Portable service contracts, capability requirements, application bootstrap, and service composition. |
| [FiBeWI](https://github.com/iobewi/fibewi) | OTA lifecycle, transactions, validation policy, and the OTA HTTP service. |
| `iobewi::http` (first component) | Shared HTTP dispatcher for TLS connections and application/service routes. Authentication policy and route migration are still pending. |
| [ConfigSpace Manager](https://github.com/iobewi/config-space-manager) | Logical persistent configuration spaces independent of a particular NVS backend. |
| [iobewi-esp](https://github.com/iobewi/iobewi-esp) | Framework ESP adapter: TLS listener first; further hardware capabilities are planned. It uses the existing [espbewi](https://github.com/iobewi/espbewi) hardware libraries. |
| [embewi-agent](https://github.com/iobewi/embewi-agent) | Application behavior and its own HTTP endpoints; consumes framework services. |

The first execution target uses Embassy with the existing ESP runtime. `iobewi` is a service framework, not a replacement RTOS kernel. Scheduling and timing constraints remain the responsibility of the runtime and platform integration.

## Design constraints

- Keep service interfaces independent of ESP partition, peripheral, and NVS types.
- Preserve a single physical flash owner when storage and OTA share hardware.
- Expose the administrative API through HTTPS; never fall back to plaintext when the TLS identity is unavailable.
- Stream OTA request bodies into the update backend without buffering a whole image in RAM.
- Separate time-critical application tasks from potentially blocking network and flash operations.
- Keep the one-shot provisioning surface separate from the runtime administrative API.

## First milestones

1. Define the portable TLS listener contract and shared HTTP dispatcher without ESP dependencies. Initial code is present; integration validation is in progress.
2. Let FiBeWI supply its OTA routes to the shared HTTP server.
3. Connect the existing ESP adapter and migrate `embewi-agent` to framework services.
4. Build the same application against a second platform adapter to validate portability.

No milestone is marked complete until the code and its target build demonstrate it.
