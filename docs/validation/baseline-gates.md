# Validation baseline gates

These gate identifiers are reusable references. A task runs the gates affected by its
scope instead of repeating the complete historical validation campaign.

## BG-ESP-S3

Purpose: prove the validated ESP32-S3 platform composition remains operational.

Minimum evidence:

- target/Agent build succeeds;
- device boots;
- Agent health/info endpoints respond as expected;
- HTTPS/auth path works and plaintext management HTTP remains disabled by the product composition;
- heartbeat and log WebSocket paths remain live when those paths are in scope.

Hardware required: yes for the complete gate.

## BG-AGENT-OTA

Purpose: preserve Agent A/B OTA and bootloader-owned lifecycle.

Minimum evidence:

- prepare/write/activate;
- reboot into candidate;
- PendingVerification/confirmation;
- rollback on failed confirmation;
- active Workload RuntimeApi compatibility guard;
- OTM1/bootloader partition ownership unchanged.

Hardware required: yes for final qualification.

## BG-WORKLOAD-OTA

Purpose: preserve OTM2 Workload A/B OTA.

Minimum evidence:

- status/prepare/streamed write;
- SHA-256 validation and resume/range behaviour;
- Staged survives reset;
- supersession invalidates prior staged metadata before overwrite;
- corrupted candidate cannot activate;
- OTM2 recovery is restart-safe.

Hardware required: yes for final qualification.

## BG-NATIVE-RUNTIME

Purpose: prove a separately built native Workload is genuinely executed and supervised.

Minimum evidence:

- IWNI candidate activation executes downloaded native code;
- log/time/control services work through the Workload ABI/SDK;
- Healthy permits confirmation to Valid;
- Valid Workload restarts after reboot and offline;
- A -> B activation and rollback stop/reload real binaries;
- invalid/corrupt target/API/image never jumps;
- faulty Workload does not make Agent unrecoverable;
- S20 boot guard/quarantine behaviour remains intact.

Hardware required: ESP32-S3.

## BG-STORAGE

Purpose: preserve storage ownership and isolation.

Minimum evidence:

- exactly one physical ESP flash owner / SharedFlash;
- `multicore_auto_park` preserved for native multicore operation;
- partition lookup and bounds validation;
- Workload writes cannot touch Agent/OTM1/NVS/bootloader/table regions;
- missing Workload partitions report unsupported capability rather than inferred free space.

Hardware required: host isolation tests plus hardware smoke when storage code changes.

## BG-USB-MSC

Purpose: qualify the supported read-only USB Mass Storage composition, without
claiming complete BOT/SCSI conformance beyond the class README's supported subset.

Minimum evidence:

- portable protocol tests and the supported target build succeed;
- a physical host enumerates the 08/06/50 class and reads the configured INQUIRY
  identity, capacity and write-protected medium information;
- supported READ(10) transfers deliver known sectors, with the documented sense
  and command status behaviour for unsupported commands and invalid ranges;
- unavailable reads follow the caller-selected retry/fallback policy; a bounded
  product policy is exercised with a stalled source;
- disconnect/reconnect ends and begins consumer sessions, including source rebasing
  when the composition uses a session-dependent source;
- file discovery/mount works when a virtual filesystem is part of the composition.

Hardware required: yes for the complete gate. Host tests do not prove physical USB
compatibility. Product prebuffer, networking, audio playback and reader-specific
acceptance remain in the product's canonical validation instructions.

## BG-WIFI-APSTA

Purpose: qualify temporary provisioning AP without hiding station disruption.
ADR-0016 records the current dependency blocker. A compile-only experiment is
not a complete gate PASS.

Minimum evidence:

- one radio owner serializes station/AP operations, including failures/cancellation;
- S3 firmware links; record dependency lock, ELF digest, radio channels and budgets;
- clients associate, obtain DHCP leases and reach only the AP provisioning page;
- activate/stop AP during nominal streaming: station association and established
  TCP connections survive (a reconnect is a FAIL for this criterion);
- AP clients/channel behavior on station reconnect and different channels recorded;
- Improv and identical-credential keep-link preserved; new-credential validation
  before commit and restoration after failure remain operational;
- explicit stop and monotonic expiry end advertising and revoke all AP services,
  including old handles across restarts; observe unexpected radio failures;
- AP HTTP is an explicit interface-local exception: STA HTTP remains inaccessible
  and AP routes do not expose the administration router;
- bounded clients/leases, DHCP renewal/expiration/exhaustion and packet/buffer bounds;
- heap and stack availability measured before/during/after repeated AP cycles.

Hardware required: ESP32-S3 and Wi-Fi clients. Report first-milestone radio results
separately from later DHCP/provisioning qualification. Do not infer physical
coexistence from portable fake-transport tests or a static-IP page.

For the dormant-APSTA candidate in ADR-0016 option 4, separately record:

- unchanged-mode station association and ONE established TCP session across cycles;
- AP beacons/channel and idle power during dormant AND active phases;
- already-associated client state/access after credential/SSID changes and behavior
  of new clients using the previous credentials; max-connections enforcement;
- independent service revocation evidence before any production adoption.

A dormant radio does not meet the advertising-stopped criterion above. Selecting
option 4 requires explicit revision of that criterion and its security/power gates.
Selecting option 3 (station interruption accepted) requires a revised gate stating
that disruption and successful recovery are allowed; it cannot pass the current
uninterrupted-association/TCP criterion by relabeling a reconnect.
