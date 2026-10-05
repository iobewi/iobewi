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
