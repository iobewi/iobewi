---
layer: platform-adapter
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-ws2812

## Summary

ESP RMT/WS2812 rendering of the portable semantic status indicator.

## Responsibilities

Publish the latest status through shared atomic state and render one WS2812 using GRB order with target-specific configurable pin capability lists.

## Non-responsibilities

Choosing product status transitions, board pin configuration, provisioning policy and generic display hardware support.

## Architecture

This concrete driver lives at `drivers/led/ws2812/esp32` and implements the
semantic contract at `drivers/indicator/core`. The portable contract remains
hardware-independent; it is not a generic LED API. Product startup supplies RMT and the configured pin and spawns the concrete led_task.

## Public API

`EspStatusIndicator` implements StatusIndicator and chip-gated StatusIndicatorCapabilities. `led_task(RMT, AnyPin) -> !` renders status; per-chip GPIO lists and selected `STATUS_LED_GPIO_NUMBERS` expose capabilities.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Validation

Build both chip features in `targets/esp32`; compile-time assertions guard
per-chip lists. The locked/latest ESP CI matrix checks both features.
BG-ESP-S3 and a visible LED smoke test validate colour/order/timing when the
renderer changes.

Existing hardware evidence is recorded in `src/lib.rs`: the S3 board was checked
visually for GRB order (RGB produced incorrect orange/green colours); the board
uses GPIO48. This is historical code-recorded evidence, not a new hardware run
or a C3 qualification. For the package/path rename, `src/lib.rs` is byte-for-byte
unchanged, including patterns, timings, GPIO lists and error handling.

## Known limitations

One global indicator state and one LED renderer. Booting is white steady, Ready blue slow blink, Scanning blue fast blink, Connecting orange fast blink, Online green steady, Failed red blink. Initialization failure logs and parks the task; it does not stop firmware. Capability lists describe SoC possibilities, not all board wiring/electrical constraints.

## Package migration

The concrete package was renamed from `iobewi-esp-indicator` to
`iobewi-esp-ws2812`, and moved from `drivers/indicator/esp32` to
`drivers/led/ws2812/esp32`. Rust imports now use `iobewi_esp_ws2812`.
This is a breaking package/import rename for Git consumers; no compatibility
package is provided. `EspStatusIndicator`, `led_task`, the GPIO capability lists
and both chip features retain their names and behaviour.

When upgrading, rename the dependency and imports, pin all direct IOBEWI Git
dependencies to the same full commit containing this rename, and regenerate the
product Cargo.lock. Path consumers must also update the path. Consumers staying
on an older revision continue to use the old package/import names.

Known consumer audit at the rename baseline:

- `embewi-agent` main (`4d69119862c110f5998d4457aa05124964dea873`) uses the old
  dependency and three imports. Its pinned revision remains usable. The product
  upgrade is tracked separately in
  [embewi-agent #16](https://github.com/iobewi/embewi-agent/issues/16).
- `streambewi` main (`7590b4b9f5aa1baabd3bd35dd071098c2809bdb2`) has no
  dependency/import of this concrete driver.

## Related components

`drivers/indicator/core`, `drivers/net/wifi/esp32` and the product composition root.
