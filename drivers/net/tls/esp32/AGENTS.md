# Agent Context — iobewi-esp-tls

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-tls`
- Path: `drivers/net/tls/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

ESP TLS transport and MbedTLS integration: global TLS instance, hardware entropy/time hooks, Embassy DNS/TCP/TLS dialer, TLS listener and secure session stream.

## Owns

- Provide ESP/MbedTLS mechanics behind portable TLS/network contracts.
- Provide feature-selected ESP32-C3/S3 integration and optional Embassy networking transport.

## Does not own

- Does not own certificate/CA persistence or NVS keys.
- Does not own SNTP policy, HTTP routes, reconnect/backoff policy or application composition.

## Architecture position

Platform adapter under `drivers/net/tls/esp32`; portable identity/trust policy remains in `crypto/*`, `net/tls/core` and `net/tls/service`.

## Public contracts

Feature `esp32c3` or `esp32s3` selects hardware integration; `embassy-net` enables DNS/TCP/TLS network transport. No chip feature is enabled by default.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- `BG-ESP-S3`

## Known limitations

ESP32-S3 MbedTLS uses the GCC path because the CMake/clang path mishandles the Xtensa four-component target triple. This is a build-toolchain constraint, not a TLS protocol distinction.

## Related components

- `crypto/mbedtls`
- `net/tls/core`
- `net/tls/service`
- `drivers/net/tcp/esp32`

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: root `AGENTS.md`, `ARCHITECTURE.md`, `INVARIANTS.md`, and referenced contracts/ADRs/gates.
