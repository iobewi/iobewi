---
layer: portable-service
status: implemented
invariants:
  - INV-001
  - INV-003
  - INV-009
  - INV-017
gates:
  - BG-AGENT-OTA
---

# iobewi-ota-http

## Summary

Portable HTTP surface for resident firmware prepare, streamed write and activation.

## Responsibilities

Mount authenticated OTA routes, validate upload metadata/ranges and stream image bytes through the caller backend without buffering the entire artifact.

## Non-responsibilities

Physical slot selection, flash/NVS mechanics, TLS listener setup and immediate hardware reset.

## Architecture

HTTP adapter above firmware/update and net/http/server. The caller supplies transaction effects and deferred reboot.

## Public API

`routes<B, R>` mounts relative `/prepare`, `/write`, `/activate`. `ControlBackend`, `WriteBackend`, `RebootPort`, `OtaWrite` and request/result/error types define effects. `prepare_response` and `activate_response` expose response construction; activation returns a reboot flag.

## Invariants

- `INV-001`
- `INV-003`
- `INV-009`
- `INV-017`

## Validation

`cargo test -p iobewi-ota-http`; run BG-AGENT-OTA for route/lifecycle changes, including streamed resume and deferred reboot on hardware.

## Known limitations

Routes must be nested under the product API prefix and served over its secured listener. Backend supplies authorization. Write uses a 1024-byte scratch buffer and u32 image/range limits. Received bytes and durable written bytes differ for resume. RebootPort must defer reset long enough to send the response; the handler schedules it before returning that response. Resident OTA only, not Workload OTA.

## Related components

`firmware/update`, `net/http/server`, `firmware/esp32`; Workload routes live in `workload/http`.
