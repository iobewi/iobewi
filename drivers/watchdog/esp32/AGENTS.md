# Agent Context — iobewi-esp-watchdog

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-esp-watchdog`
- Path: `drivers/watchdog/esp32`
- Layer: `platform-adapter`
- Status: `implemented`

## Role

Low-level TIMG0 watchdog control for a caller-owned boot verification window.

## Owns

Arm the first hardware watchdog stage, feed it and disable it on request.

## Does not own

OTA lifecycle policy, selecting deadlines, self-check decisions and RTC deferred reset scheduling.

## Architecture position

Platform mechanism called by resident firmware/OTA composition; it consumes no portable OTA state.

## Public contracts

`arm_ms(timeout_ms)`, `feed()` and `disable()` operate the TIMG0 watchdog.

## Invariants

- No additional crate-specific invariant is declared; repository-wide invariants still apply.

## Modification context

See the canonical README and implementation.

## Required validation

Build selected chip; BG-AGENT-OTA hardware tests must demonstrate expiry, feeding and disarming in the actual initialization order.

## Known limitations

esp_hal::init disables watchdogs. Initialize TIMG0 for esp_rtos before arm_ms; constructing the timer group afterwards resets the peripheral and clears the watchdog. The caller owns coordination and feeding. RTC system-reset scheduling is a separate capability in arch/esp32/reset.

## Related components

`arch/esp32/reset`, `firmware/esp32`, product boot/self-check orchestration.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
