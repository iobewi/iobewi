---
layer: platform-adapter
status: implemented
invariants: []
gates:
  - BG-ESP-S3
---

# iobewi-esp-indicator

## Summary

ESP RMT/WS2812 rendering of the portable semantic status indicator.

## Responsibilities

Publish the latest status through shared atomic state and render one WS2812 using GRB order with target-specific configurable pin capability lists.

## Non-responsibilities

Choosing product status transitions, board pin configuration, provisioning policy and generic display hardware support.

## Architecture

Platform implementation of drivers/indicator/core. Product startup supplies RMT and the configured pin and spawns the concrete led_task.

## Public API

`EspStatusIndicator` implements StatusIndicator and chip-gated StatusIndicatorCapabilities. `led_task(RMT, AnyPin) -> !` renders status; per-chip GPIO lists and selected `STATUS_LED_GPIO_NUMBERS` expose capabilities.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Validation

Build selected chip; compile-time assertions guard per-chip lists. BG-ESP-S3 and a visible LED smoke test validate colour/order/timing when renderer changes.

## Known limitations

One global indicator state and one LED renderer. Booting is white steady, Ready blue slow blink, Scanning blue fast blink, Connecting orange fast blink, Online green steady, Failed red blink. Initialization failure logs and parks the task; it does not stop firmware. Capability lists describe SoC possibilities, not all board wiring/electrical constraints.

## Related components

`drivers/indicator/core`, `drivers/net/wifi/esp32` and the product composition root.
