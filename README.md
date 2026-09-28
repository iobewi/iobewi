# IOBEWI

IOBEWI is the company behind this portable Rust framework for embedded services. This repository contains the framework; the company name also covers its other projects.

> **Status:** HTTP, HTTPS, ConfigSpace and OTA provide separate portable crates. Service composition, OTA routes, platform ports beyond ESP and further application migrations remain in progress.

## Goal

`iobewi` defines how embedded applications obtain services through platform-independent contracts. An application such as [embewi-agent](https://github.com/iobewi/embewi-agent) supplies its own behavior and consumes those services. Platform adapters provide the hardware capabilities required to run them on ESP, RP2350, Teensy, or another supported target.

This repository is a Cargo workspace: each service lives in its own crate under
`services/` with its own `Cargo.toml`. Current members are [`iobewi-http`](services/http), the portable HTTP
dispatcher and outbound HTTP/WebSocket client; [`iobewi-https`](services/https), its TLS-only entry point; and
[`iobewi-config-space`](services/config-space), configuration ownership and quotas; and
[`iobewi-ota`](services/ota), transactional firmware updates.

The intended dependency direction is:

```text
embewi-agent (application)
    -> iobewi (service contracts and composition)
        -> services (OTA, HTTP, HTTPS, ConfigSpace, ...)
        -> platform adapter (iobewi-esp, future RP2350/Teensy adapters)
            -> HAL, network and execution runtime
```

The application should not need an ESP-specific type to use OTA, HTTP, storage, or the watchdog. A service may require capabilities that a given board does not provide; the composition must make that requirement explicit.

## Components and ownership

| Component | Responsibility |
| --- | --- |
| `iobewi` | Portable service contracts, capability requirements, application bootstrap, and service composition. |
| `services/ota` (`iobewi-ota`) | OTA transactions, durable metadata, streaming writes, validation policy and restart recovery. |
| `services/http` (`iobewi-http`) | HTTP dispatcher, reusable outbound JSON POST/response framing and WebSocket client protocol over a connected stream. |
| `services/tls` (`iobewi-tls`) | Portable certificate/CA policy and optional authenticated provisioning API mounted by the application on its HTTPS router. |
| `services/log-stream` (`iobewi-log-stream`) | Bounded log capture, WebSocket forwarding and reconnect policy over a supplied secure transport. |
| `services/ntp` (`iobewi-ntp`) | SNTP synchronization and an epoch clock over Embassy networking; server and timing policy are supplied at startup. |
| `services/https` (`iobewi-https`) | TLS listener contract: only a completed handshake reaches the HTTP dispatcher. |
| `services/config-space` (`iobewi-config-space`) | Logical persistent configuration spaces, quotas and generations independent of the physical backend. |
| [iobewi-esp](https://github.com/iobewi/iobewi-esp) | ESP implementation workspace: HTTP over TCP, HTTPS over MbedTLS, flash, NVS, OTA, Wi-Fi and bootloader. |
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

1. Separate the portable HTTP dispatcher and HTTPS listener contract without ESP dependencies. Initial code is present; integration validation is in progress.
2. Connect `iobewi-ota` to the shared HTTP server; the portable OTA routes remain to be implemented.
3. Connect the existing ESP adapter and migrate `embewi-agent` to framework services.
4. Build the same application against a second platform adapter to validate portability.

No milestone is marked complete until the code and its target build demonstrate it.
