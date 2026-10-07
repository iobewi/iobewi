---
layer: validation-tool
status: experimental
invariants:
  - INV-001
gates:
  - BG-WIFI-APSTA
---

# AP31 qualification experiment

## Summary

Isolated ESP32-S3 firmware comparing STA/APSTA/STA mode changes and
same-mode dormant/active APSTA reconfiguration with
`esp-radio =1.0.0-beta.1`. First qualification milestone of issue #31;
**not production APSTA support**.

## Responsibilities

- Own one controller and serialize all radio changes in one task.
- Instantiate separate station DHCP-client and AP static-IP stacks/runners.
- Exercise three mode cycles and report link/config/association and free heap.
- Provide a raw TCP echo on STA and a minimal test page only on AP.

## Non-responsibilities

No provisioning credential endpoint, DHCP server, captive DNS, config-space,
flash writes, Board changes or production AP handles. No claim that station
connections survive. No production lifecycle revocation or expiration service.

## Architecture

Independent experiment workspace. Production driver and portable services are
unchanged. See [ADR-0016](../../../docs/decisions/ADR-0016-wifi-ap-qualification.md)
for the upstream stop/start blocker and proposed contracts.

## Public API

No reusable API. `run.sh` builds a local test binary, using required
`AP31_STA_SSID`, `AP31_STA_PASSWORD`, and `AP31_AP_PASSWORD` environment variables.
The optional mode argument defaults to `locked`, which builds the committed
experiment lockfile. `latest` first runs `cargo +esp update` in this independent
workspace, then builds with `--locked` against that newly resolved lockfile.
CI passes its matrix mode; local `latest` updates the experiment lockfile, which
must not be committed accidentally. Other mode values are rejected.
Both test networks use WPA2. Secrets are embedded in the local firmware; do not
publish the ELF, flash image or build artifacts. Never use production credentials.
A unique temporary AP password is required; no MAC-derived secret or open fallback.
Only generic diagnostic flags are logged, never credentials or panic details.

### Lifecycle and variants

`run.sh MODE VARIANT` accepts `locked`/`latest` and
`mode-transition` (default)/`dormant-apsta` respectively. CI updates the independent
workspace once in the latest leg, then builds both variants against that resolution.

- `mode-transition`: starts in STA and repeats STA -> APSTA -> STA three times.
  After each mode switch, it waits for the OLD DHCP config to go down (20-second
  timeout) BEFORE reassociating and waiting for a new config (20 seconds each).
  Failure to observe config-down stops the experiment rather than claiming recovery
  using a stale lease. Reassociation is recovery, never a keep-link PASS.
- `dormant-apsta`: starts directly in APSTA, with a hidden SSID, independent WPA2
  secret and one allowed client. The caller supplies `AP31_DORMANT_PASSWORD`, distinct
  from `AP31_AP_PASSWORD`. After initial STA association/DHCP, three cycles alternate
  real AP and dormant config in the SAME radio mode, with identical STA config.
  It NEVER reconnects the station during these cycles. Beacons remain active and
  final state is dormant, not stopped. One client is a test setting, not a verified
  minimum; zero connections is not assumed valid.

Both wait 30 seconds after initial station readiness for the TCP probe. Active and
inactive-requested intervals each last 32 seconds including a 2-second settle.
A failure panics; power off if the run stops progressing. This diagnostic firmware
is not a safety-bounded production AP service.

`ap_link` is radio availability, `ap_ip` is configured static-IP availability;
IP alone does not prove clients can associate. Heap readings do not measure stack
high-water marks or total PHY/static memory. A current STA DHCP lease is likewise
insufficient proof of an uninterrupted link.

AP: `172.23.241.1/24`, at most two radio clients, preferred channel 1. The channel
may follow an associated station: record real STA/AP channels and client behavior
on different upstream channels, including station loss and reconfiguration.
The less common `172.23.241.0/24` is still not collision-free: verify upstream/VPN
routes before testing and do not use it when the station subnet overlaps.

Fixed resources: 128 KiB heap; STA `StackResources<4>`, AP `StackResources<3>`;
one runner per interface; TCP echo and AP page each use 1024-byte RX/TX buffers
plus 256/128-byte request buffers. These experiment budgets do not change Board
budgets. AP socket set does NOT reserve an implemented DHCP server.

## Invariants

- `INV-001`: target-local experiment, no platform dependency added to portable code.

## Validation

With Xtensa toolchain/export environment installed:

```sh
# Supply temporary test values locally; do not commit them.
export AP31_STA_SSID AP31_STA_PASSWORD AP31_AP_PASSWORD
# Baseline source-confirmed destructive mode transition:
bash tools/experiments/ap31/run.sh locked mode-transition
# Same-mode candidate, with an independent temporary dormant secret:
export AP31_DORMANT_PASSWORD
bash tools/experiments/ap31/run.sh locked dormant-apsta
# Flash locally with espflash, using the UART0 console on GPIO43/44.
espflash flash --monitor tools/experiments/ap31/target/xtensa-esp32s3-none-elf/release/ap31-qualification
```

Read the station IP on UART. During the initial 30-second wait, run:

```sh
python3 tools/experiments/ap31/tcp_probe.py STATION_IP
```

The probe opens exactly ONE TCP session. An EOF, timeout or socket failure is a
failure; reconnecting does not satisfy continuity. Initial connection errors must
be distinguished from a session broken at the radio mode transition.

During APSTA, associate a client using the temporary password. **Manually** set
client IP `172.23.241.2/24`, no gateway/DNS, and open `http://172.23.241.1/`.
This proves only static-IP access, not DHCP acceptance or provisioning. The fixture
answers only a simple `GET /` test request; it is not the product's HTTP router.
Verify station IP port 80 remains closed. In `mode-transition`, verify the AP
disappears on return to STA; in `dormant-apsta`, record continuing beacons.
Capture timestamps, external association/TCP evidence and memory readings for each
cycle. For `dormant-apsta`, leave an active client associated when the dormant config
returns: record whether association and HTTP access persist, then attempt a fresh
association using the previously active credentials. The page deliberately remains
running to expose retained-client access. Do not equate hidden SSID or changed
password with deauthentication. Use an external scanner to record AP beacons/channel;
UART reports station channel only. Compare current/power with `mode-transition`'s
station-only intervals; heap readings cannot prove low power.

Run the TCP probe through all cycles (default 300 seconds), synchronize with UART
phase markers and require the firmware completion marker. A finite-duration TCP
PASS before cycles complete is insufficient evidence. Capture AP channel/client
effects during same-mode changes and subsequent station loss/reconnect testing.

Software evidence and hardware evidence must be reported separately. The entire
[BG-WIFI-APSTA](../../../docs/validation/baseline-gates.md#bg-wifi-apsta) remains
pending until physical measurements exist; this fixture covers only its first
radio qualification subset. Improv, DHCP lease behavior, credential commit/restore,
forced expiration and product streaming need later integration fixtures.

## Known limitations

`set_config` stops the whole radio when changing mode. The default variant
reproduces that behavior; `dormant-apsta` evaluates same-mode changes separately.
Same-mode errors can still stop both interfaces. Dormant mode has persistent
beacons/channel coupling/power overhead and unqualified client deauthentication. An explicit reconnect after
stop is recovery, not successful keep-link. No hardware PASS has been recorded.
The test page task is not a revocable production service; old sessions must be
handled by the eventual adapter. Firmware remains isolated from the default build.

## Related components

- `drivers/net/wifi/esp32`
- `net/wifi/core`, `net/wifi/manager`
- ADR-0016 and issue #31

The independent lockfile and automatic experiment builds are temporary qualification
costs. When #31's strategy is accepted, remove/replace the fixture or move its radio
qualification builds to a manual workflow; do not retain duplicate builds indefinitely.
