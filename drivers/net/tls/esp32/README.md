---
layer: platform-adapter
status: implemented
invariants:
  - INV-001
gates:
  - BG-ESP-S3
---

# iobewi-esp-tls

## Summary

ESP TLS transport and MbedTLS integration: global TLS instance, hardware entropy/time hooks, Embassy DNS/TCP/TLS dialer, TLS listener and secure session stream.

## Responsibilities

- Provide ESP/MbedTLS mechanics behind portable TLS/network contracts.
- Provide feature-selected ESP32-C3/S3 integration and optional Embassy networking transport.

## Non-responsibilities

- Does not own certificate/CA persistence or NVS keys.
- Does not own SNTP policy, HTTP routes, reconnect/backoff policy or application composition.

## Architecture

Platform adapter under `drivers/net/tls/esp32`; portable identity/trust policy remains in `crypto/*`, `net/tls/core` and `net/tls/service`.

## Public API

Feature `esp32c3` or `esp32s3` selects hardware integration; `embassy-net` enables DNS/TCP/TLS network transport. No chip feature is enabled by default.

## Invariants

- `INV-001`

## Validation

- `BG-ESP-S3`

## Known limitations

ESP32-S3 MbedTLS uses the GCC path because the CMake/clang path mishandles the Xtensa four-component target triple. This is a build-toolchain constraint, not a TLS protocol distinction.

## Related components

- `crypto/mbedtls`
- `net/tls/core`
- `net/tls/service`
- `net/tcp`

