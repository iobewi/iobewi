# Agent Context — iobewi-esp-ws2812

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-ws2812`
- Path: `drivers/led/ws2812/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

ESP RMT/WS2812 rendering of the portable semantic status indicator.

## Owns

Publish the latest status through shared atomic state and render one WS2812 using GRB order with target-specific configurable pin capability lists.

## Does not own

Choosing product status transitions, board pin configuration, provisioning policy and generic display hardware support.

## Architecture position

This concrete driver lives at `drivers/led/ws2812/esp32` and implements the
semantic contract at `drivers/indicator/core`. The portable contract remains
hardware-independent; it is not a generic LED API. Product startup supplies RMT and the configured pin and spawns the concrete led_task.

## Public contracts

`EspStatusIndicator` implements StatusIndicator and chip-gated StatusIndicatorCapabilities. `led_task(RMT, AnyPin) -> !` renders status; per-chip GPIO lists and selected `STATUS_LED_GPIO_NUMBERS` expose capabilities.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Modification context

See the canonical README and implementation.

## Required validation

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

## Related components

`drivers/indicator/core`, `drivers/net/wifi/esp32` and the product composition root.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
