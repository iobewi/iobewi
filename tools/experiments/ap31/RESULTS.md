# AP31 qualification evidence

Base: `345378ed1ae11a4814c32ee513f20438291cfba7`.
Branch: `feat/31-apsta-qualification`.

## Software results

- Source inspected: pinned `esp-radio 1.0.0-beta.1`, especially
  `WifiController::set_config` and `stop_impl`. Mode change stops the entire radio;
  no public independent AP stop/start was found. This blocks keep-link acceptance.
- Local **release ELF build/link PASS**, ESP32-S3 / Xtensa, with the experiment
  committed lockfile and placeholder WPA2 credentials. Xtensa Rust
  `1.97.0-nightly (8ea53bcd7)`, esp-hal 1.2.2, esp-radio 1.0.0-beta.1,
  esp-rtos 0.4.0, embassy-net 0.9.1. Linker reports a RWX LOAD segment; this fixture
  does not claim memory isolation or security qualification from ELF permissions.
- Existing Wi-Fi manager host regressions: **16 PASS** (including connection before
  commit, failed-commit restoration and failed reprovision restoration).
- Existing ESP keep-link decision regressions: **2 PASS**.
- Portable Wi-Fi core compiles/tests; it currently contains zero tests.
- Rust formatting, documentation generation/check, strict MkDocs site build and
  diff whitespace checks: **PASS**.

The firmware build first needed a local Xtensa rust-src installation repair
(missing library files). That environment repair is not a repository change.

## Hardware results

**NOT RUN**: no ESP32-S3 or Wi-Fi client attached to this execution environment.
No claim of DHCP, production provisioning, uninterrupted streaming, APSTA radio
qualification or full BG-WIFI-APSTA PASS. Follow README's UART/static-IP/TCP
procedure and attach real timestamps, channels and memory readings.

## Scope remaining

Issue #31 remains open. Resolve the radio API prerequisite or explicitly revise
the uninterrupted-station requirement before implementing the production port,
coordinator, adapter, DHCP server and provisioning listener. ADR-0016 describes
proposed lifecycle/security contracts; they are not an implemented public API.
