# ADR-0016 — Temporary provisioning AP: qualification prerequisite

Status: proposed; **production APSTA integration blocked** by the current radio API.
Related: [issue #31](https://github.com/iobewi/iobewi/issues/31).
Base: `345378ed1ae11a4814c32ee513f20438291cfba7`.

## Context and evidence

IOBEWI currently owns one controller in `drivers/net/wifi/esp32::Radio` and
one station stack. Lazy initialization and unchanged-credential connection reuse
must survive AP work. No new persistence is needed to activate an AP.

Source inspection of the locked **esp-radio 1.0.0-beta.1** establishes:

- `wifi::Config::AccessPointStation(StationConfig, AccessPointConfig)` exists.
- `Interface::station()` and `Interface::access_point()` provide distinct,
  exclusively acquired network devices. Two Embassy stacks/runners are possible.
- `WifiController::set_config` compares the old and new radio modes; **if they
  differ it calls `Self::stop_impl()` before setting the mode and restarting**.
- `stop_impl` calls `esp_wifi_stop`. STA -> APSTA and APSTA -> STA are therefore
  whole-radio stop/start operations, not additive activation/deactivation.
- Configuration failure resets the mode to NULL and stops the controller. It
  can invalidate BOTH interfaces, even when requesting only an AP change.
- The documented caller must explicitly call `connect_async` after configuring.
  A retained stack/DHCP address is not evidence of a surviving association.
- The public controller API exposes no independent AP start/stop operation.

Primary source: [set_config and stop_impl at the exact dependency tag](https://github.com/esp-rs/esp-hal/blob/esp-radio-v1.0.0-beta.1/esp-radio/src/wifi/mod.rs).
The downloaded crate source and `targets/esp32/Cargo.lock` are the executable
qualification baseline; the crate is pinned, not an API guessed from latest docs.

This fails the issue's uninterrupted station acceptance criterion at the mechanism
level. Supporting an AP while a station is stopped would not validate APSTA.
Keeping APSTA permanently configured is also not a solution to a temporary AP:
the existing API does not independently stop advertising the AP.

## Decision at this milestone

Do not modify the production station driver, Board, entry, durable Wi-Fi manager
or config-space. Add the isolated [AP31 experiment](../../tools/experiments/ap31/README.md)
to reproduce the mode transition using one controller and two networks.
Do not implement a production AP facade or DHCP on top of a destructive transition
while claiming keep-link support. Issue #31 remains open. The experiment is NOT a
qualified AP implementation, provisioning UI or production API.

Before proceeding, select and validate one of:

1. An upstream public radio API that starts/stops AP independently and preserves
   the station, including failure semantics. Prefer this route.
2. A reviewed dependency patch exposing such a capability, with hardware proof
   and a deliberate support/upgrade policy. Do not call private FFI in IOBEWI.
3. An explicitly revised product requirement accepting station interruption.
   This changes issue #31's scope and must not be described as keep-link APSTA.

## Proposed portable contract for the next milestone

Keep `WifiTransport` unchanged. A distinct `WifiAccessPoint` port has associated
`Error` and opaque `NetworkHandle` types and fallible asynchronous `start(config)`
and `stop()` operations. It reports a synchronous state snapshot and an optional
session handle. No ESP/Embassy types occur in portable APIs.

| State | Radio | Network handle | Transition |
|---|---|---|---|
| Stopped | AP inactive | None | explicit valid start -> Starting |
| Starting | activating | None | AP radio AND IP readiness -> Ready; error -> Faulted |
| Ready | AP active | current session only | stop/expiry -> Stopping; loss -> Faulted |
| Stopping | disabling | None | radio stopped and services ended -> Stopped; error -> Faulted |
| Faulted | availability uncertain | None | explicit recovery stop; no implicit exposure |

Static IP configuration alone does not imply AP radio readiness. DHCP task
readiness is a separate service condition. Observation must include unexpected
radio loss, not merely the last requested state.

Starting the same config while Ready is idempotent and **does not extend** the
original exposure deadline. A different config returns Busy; caller must stop
first. Stop while Stopped succeeds. Errors remain typed and observable. No success
may be reported until the hardware acknowledges the requested lifecycle action.

One radio owner serializes scan, station connect/reconfigure, AP start/stop and
expiry. Its queue/actor or exclusive borrow must remain held across each operation's
await points. Independent freely mutable station and AP controllers are forbidden.
Provisioning callers send commands to that owner; the owner does not hold a mutex
across indefinite station monitoring waits. Cancellation of a partially completed
operation must invalidate availability and arrange cleanup before the next command.

A handle includes a non-reused session identity and a revocation capability. Stop
revokes it **before** awaiting radio changes: DHCP/listener/socket tasks must abort
and acknowledge closure. Old handles must never become valid again after restart.
A copyable raw Embassy stack plus generation metadata with no enforced check is
insufficient. Future adapter construction must enforce revocation at service I/O
boundaries. These semantics require tests before accepting the production port.

Expiration uses monotonic time and is bounded by a validated nonzero application
TTL. The coordinator must service it even when no UI request arrives, reject expired
sessions, and retry/escalate a failed radio stop; a deadline is not proof the radio
stopped. Framework provides mechanics; product decides activation, duration,
SSID/security, routes and stop on successful provisioning. Station loss never
implicitly activates the AP.

## DHCP, UDP and resource plan

Preliminary dependency inspection (downloaded crate sources):

| Candidate | Findings | Follow-up |
|---|---|---|
| `edge-dhcp 0.8.0` | no_std/no-alloc codec and `Server<F, const N>` with fixed-capacity leases; optional I/O uses edge-nal and Embassy time | preferred evaluation candidate; test offered-address reservation, client identity, exact expiration boundary, DECLINE quarantine and renewal before adopting |
| `dhcproto 0.15.0` | no_std codec, allocated message/options; not a ready bounded lease service | broader codec than needed; would require a lease engine and I/O |
| `embassy-net 0.9.1 dhcpv4` | station DHCP client | cannot serve AP leases |

Primary dependency sources: [edge-dhcp](https://github.com/sysgrok/edge-net),
[dhcproto](https://github.com/bluecatengineering/dhcproto). No dependency is selected
or server implemented by this qualification change. The next milestone must verify
UDP broadcast/unicast source/destination handling, network isolation and lease tests
before reuse or internal implementation. Existing Embassy DHCPv4 is a
**client**, not the AP server. Existing TCP/HTTP ports are useful only after an AP
network and lifecycle-safe listener are available.

Proposed initial budget: two AP clients and two leases, one DHCP UDP socket, one
HTTP TCP socket and an ARP/network reserve; a separate AP stack with at least three
socket resources. Station resources remain independently budgeted. A fixed lease
table maps client identity to address, tracks offered/committed leases and monotonic
expiration. DISCOVER/REQUEST/renew/RELEASE/DECLINE, collisions, invalid requests,
expiration and exhaustion need executable tests. Size packet/metadata buffers
explicitly and derive Board's RAM/resource budget from measured composition.
No unbounded per-client allocation or default 255-client radio limit.

The experiment fixes STA/AP socket sets at 4/3 and a 128 KiB heap, with two radio
clients. It measures heap free before/during/after three cycles. These are explicit
**experiment** budgets, not approved Board defaults or measured production needs.

## Security and provisioning constraints

AP configuration is supplied by the product and activation exists only in RAM.
Start/stop must make no flash or config-space writes. Use WPA2 with a unique random
per-device or user-supplied secret. MAC-only derivation is not secret. An open AP
requires an explicit short-lived physical activation policy; it is not a default.
Never log SSIDs/passwords or include them in error/debug representations.

Plain HTTP provisioning is an exception restricted to the AP network. Construct
its listener from the AP session, not a wildcard/station listener. Expose only the
small provisioning route set, never the complete administration router. Existing
station HTTPS/auth rules remain unchanged. The exception must be included in the
hardware gate; test that station cannot reach the AP HTTP listener.

Provisioning reuses the existing `WifiProvisioning` transaction: verify a successful
station connection before committing credentials; preserve restoration on failure
and identical-credential keep-link behavior. AP lifecycle cannot commit credentials.

## Qualification gate and consequences

Run [BG-WIFI-APSTA](../validation/baseline-gates.md#bg-wifi-apsta) before accepting
production APSTA support. Compile/link only proves API compatibility; radio behavior,
channel coupling, active TCP preservation, client disconnects and memory recovery
require an S3 and a real Wi-Fi client. No hardware PASS is claimed here.

After resolving the radio blocker: contracts/coordination + fake transports,
platform adapter + hardware evidence, then DHCP/IP HTTP integration, in that order.
Captive DNS/portal remain outside the initial scope. Keep issue #31 distinct from
log-policy work and stop before merge for review.
