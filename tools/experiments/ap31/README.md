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

Isolated ESP32-S3 firmware reproducing STA/APSTA/STA mode changes with
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
Both test networks use WPA2. Secrets are embedded in the local firmware; do not
publish the ELF, flash image or build artifacts. Never use production credentials.
A unique temporary AP password is required; no MAC-derived secret or open fallback.
Only generic diagnostic flags are logged, never credentials or panic details.

### Lifecycle

Station connects, waits for DHCP (20 seconds), then waits 30 seconds to attach
`tcp_probe.py`. APSTA is requested, observed after 2 seconds, then explicitly re-associated
(if necessary, 20-second timeout) to qualify actual concurrent AP/STA operation.
After a 30-second AP interval, returning to STA is followed by
an explicit station reconnect and 10-second pause. Repeat three times, then keep
AP off. A failure panics; power off the test device if the run stops progressing.
**This diagnostic firmware is not a safety-bounded production AP service.**

`ap_link` is radio availability, `ap_ip` is configured static-IP availability;
IP alone does not prove clients can associate. Heap readings do not measure stack
high-water marks or total PHY/static memory. A current STA DHCP lease is likewise
insufficient proof of an uninterrupted link.

AP: `192.168.31.1/24`, at most two radio clients, preferred channel 1. The channel
may follow an associated station: record real STA/AP channels and client behavior
on different upstream channels, including station loss and reconfiguration.
Do not select an upstream test network using the same subnet as the AP.

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
bash tools/experiments/ap31/run.sh
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
client IP `192.168.31.2/24`, no gateway/DNS, and open `http://192.168.31.1/`.
This proves only static-IP access, not DHCP acceptance or provisioning. The fixture
answers only a simple `GET /` test request; it is not the product's HTTP router.
Verify station IP port 80 remains closed and AP disappears on return to STA.
Capture timestamps, external association/TCP evidence and memory readings for each
cycle. Capture AP channel/client effects while the station is re-associated.

Software evidence and hardware evidence must be reported separately. The entire
[BG-WIFI-APSTA](../../../docs/validation/baseline-gates.md#bg-wifi-apsta) remains
pending until physical measurements exist; this fixture covers only its first
radio qualification subset. Improv, DHCP lease behavior, credential commit/restore,
forced expiration and product streaming need later integration fixtures.

## Known limitations

`set_config` stops the whole radio when changing mode. This experiment deliberately
reproduces that behavior; it does not work around it. An explicit reconnect after
stop is recovery, not successful keep-link. No hardware PASS has been recorded.
The test page task is not a revocable production service; old sessions must be
handled by the eventual adapter. Firmware remains isolated from the default build.

## Related components

- `drivers/net/wifi/esp32`
- `net/wifi/core`, `net/wifi/manager`
- ADR-0016 and issue #31
