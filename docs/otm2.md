# OTM2 — persistent record of the Workload OTA (S15)

OTM1 = the **Agent** OTA record (`iobewi-ota`, unchanged). OTM2 = the **Workload**
OTA record (`iobewi-workload-ota`, `workload/update`). They are two independent
formats: other magic, other storage, no shared transaction, **no migration**.
Slots (A/B) are local to the device: `embewi-core` never sees them. Activating an
Agent (bootloader, reboot) is not activating a Workload (the Agent's supervisor,
no reboot). Terminology: see `docs/dual-ota.md`.

S15 stores *selection state only*: which Workload is active, which candidate is
staged, which one to return to. Nothing here loads, runs or health-checks a
Workload.

## 1. Binary format (version 1)

Little-endian, packed, no implicit padding, no dependence on `repr(Rust)`.
`RECORD_LEN` = **180 bytes**.

| offset | field | size | endian | semantics |
|---|---|---|---|---|
| 0 | magic | 4 | – | `"OTM2"` |
| 4 | format_version | 1 | – | `1`; any other value is rejected |
| 5 | state | 1 | – | `0` Empty, `1` Valid, `2` Staged, `3` Activating, `4` PendingConfirmation, `5` RollingBack |
| 6 | active | 1 | – | slot selected to run: `0`=A, `1`=B, `0xFF`=none |
| 7 | candidate | 1 | – | staged slot, not yet selected: `0`/`1`/`0xFF` |
| 8 | previous_valid | 1 | – | last confirmed slot to return to: `0`/`1`/`0xFF` |
| 9 | reserved | 3 | – | zero (non-zero ⇒ malformed) |
| 12 | sequence | 4 | LE | u32, compared with serial-number arithmetic (wrap-safe) |
| 16 | meta[A] | 80 | – | slot A artifact metadata (below) |
| 96 | meta[B] | 80 | – | slot B artifact metadata |
| 176 | crc32 | 4 | LE | zlib CRC-32 (poly 0xEDB88320) of bytes `0..176` |

Slot metadata (80 bytes; an unused slot is all zero):

| offset | field | size | endian | semantics |
|---|---|---|---|---|
| 0 | digest | 32 | – | SHA-256 of the artifact (same algorithm as OTM1/the engine) |
| 32 | size | 4 | LE | artifact size in bytes (u32) |
| 36 | req_major | 2 | LE | required runtime API, major |
| 38 | req_minor | 2 | LE | required runtime API, minor |
| 40 | version | 16 | – | UTF-8, NUL padded, no embedded NUL |
| 56 | id | 24 | – | artifact id, UTF-8, NUL padded, no embedded NUL |

The CRC protects the **record**; the SHA-256 protects the **artifact**.

**Integrity checks** on decode: erased (`0xFF…`) ⇒ `Blank` (never written, not
corrupt); length; magic; version; CRC; reserved bytes; slot codes; coherence of
the slot fields with the state:

| state | active | candidate | previous_valid |
|---|---|---|---|
| Empty | none | none | none |
| Valid | some | none | none |
| Staged | any | some (≠ active) | none |
| Activating | any (the old one) | some (≠ active) | none |
| PendingConfirmation | some (running, unconfirmed) | none | any (≠ active) |
| RollingBack | some (the failed one) | none | any (≠ active): where to return |

**Example** (logical): `seq=42, state=PendingConfirmation, active=B,
previous_valid=A, candidate=none, meta[B]={id:"pod", version:"1.1.0",
sha256:…, size:8192, requires:1.3}, meta[A]={…}` — i.e. B was activated and is
running unconfirmed, A is the way back.

**Size budget**: record 180 B (20 B of framing: 16 header + 4 CRC; 160 B of
slot metadata); double copy 360 B; a raw-flash backend rounds each copy up to
its erase unit (2 × 4 KiB on ESP). No allocation in the codec.

## 2. State machine

| state | event | next | |
|---|---|---|---|
| Empty / Valid / Staged | `prepare` + verified write + `commit_staged` | Staged | allowed; an older Staged candidate is superseded |
| Activating / PendingConfirmation / RollingBack | `prepare` | – | refused `Busy` |
| Staged | `activate`, agent API satisfies requirement | Activating → PendingConfirmation | allowed |
| Staged | `activate`, requirement not met | Staged | refused `IncompatibleRuntimeApi` (nothing persisted, supervisor not called) |
| PendingConfirmation | `confirm` | Valid | allowed (the trigger — health — is a future capability) |
| Activating / PendingConfirmation / RollingBack | `rollback` | RollingBack → Valid, or Empty if nothing to return to | allowed (also completes an interrupted rollback) |
| anything else | `activate` / `confirm` / `rollback` | – | refused `WrongState` |

The engine's own vocabulary (`Staged`, `Activating`, `PendingConfirmation`,
confirmed) is reused; the only added state is `RollingBack`, needed to survive a
power cut during a rollback. "Invalid" is not a persisted state: a candidate
whose digest fails verification is never persisted.

## 3. Storage, atomicity, recovery

* **Metadata**: two copies of the record. A commit writes the copy that does
  **not** hold the newest valid record, with `sequence + 1`; the last good
  record therefore survives any interrupted write. A reader takes the newest
  valid copy. Backends implement `MetadataBackend` (read/write one copy);
  nothing in it names a platform.
* **Slot bytes**: through the shared streaming engine (`iobewi_ota::WriteSession`
  over an `ArtifactStorage` per slot): digest while writing, size check,
  resume/resync, no read-back. Only a verified write can become `Staged`.
* **Atomicity** of selection changes = one record commit each; `activate` and
  `rollback` commit *intent* first, then call the supervisor, then commit the
  settled state.
* **Recovery** (`recover()`), without any loader:

| Found | Result |
|---|---|
| both copies erased | `NoWorkload` |
| newest valid copy, other torn/corrupt | the valid one (degraded, reported) |
| state Empty / Valid / Staged / PendingConfirmation | `NoWorkload` / `Valid(slot)` / `Staged{…}` / `PendingConfirmation(slot)` |
| state Activating / RollingBack | `RollbackRequired{restore}` — complete with `rollback` |
| no valid copy, not blank | `CorruptedMetadata` — **no state is invented**; operations refuse until handled |

## 4. Compatibility

The record persists the *required* runtime API per slot. At `activate` the
engine checks `agent_api.satisfies(required)` (same major, provided minor ≥
required minor) **before** persisting or calling the supervisor. The Agent can
re-check the active Workload at boot or after its own update with
`active_requirement()`.

## 5. Source layout

`workload/update` (crate `iobewi-workload-ota`): `otm2` (codec) / `store`
(double-copy) / `machine` (policy + upload glue). Only real S15 code lives under
`workload/` (recommendation of S14, option 2). Dependencies: `iobewi-update-model`
and the shared engine `iobewi-ota`; no ESP, HTTP, TLS or embassy type.

## 6. Factory reset / erase

OTM2 does not exist in deployed devices yet; nothing changes now. Intended
policy (to confirm against the product's factory policy before freezing): a
factory erase removes both OTM2 copies and the Workload slots ⇒ `NoWorkload`.
Today's web-flasher flow erases the whole flash, which already has that effect.

## 7. ESP32 flash layout audit (no change applied)

`embewi-agent/partitions.csv` (A/B, no factory):

| name | type | offset | size |
|---|---|---|---|
| nvs | data/nvs | 0x9000 | 0x6000 (24 KiB) |
| otadata | data/ota | 0xf000 | 0x2000 (8 KiB) |
| phy_init | data/phy | 0x11000 | 0x1000 |
| ota_0 | app/ota_0 | 0x20000 | 0x180000 (1.5 MiB) |
| ota_1 | app/ota_1 | 0x1a0000 | 0x180000 (1.5 MiB) |

The table ends at `0x320000` (3.125 MiB). The file's own comment says "4 MB",
but the S3 target (ESP32-S3-N16R8) is flashed as **16 MB** (`--flash-size 16mb`):
**no Workload space exists today**. Free space after the table:

* S3, 16 MB: `0x320000..0x1000000` = 13,500,416 B (12.87 MiB) — two realistic slots fit.
* C3, 4 MB: `0x320000..0x400000` = 917,504 B (896 KiB) — not two realistic slots.

Proposal (**not applied**, needs explicit validation): append after `ota_1`,
leaving every existing offset untouched —

| name | type | offset | size |
|---|---|---|---|
| wl_meta | data (custom) | 0x320000 | 0x2000 (two 4 KiB OTM2 copies) |
| workload_a | data (custom) | 0x330000 | 0x400000 (placeholder 4 MiB) |
| workload_b | data (custom) | 0x730000 | 0x400000 (placeholder 4 MiB) |

ends at `0xB30000` (~11.2 MiB; ~4.8 MiB spare). Impact: OTA Agent none (ota_0 /
ota_1 / otadata / NVS unchanged); the partition-table binary and the factory
image change; existing devices keep their old table until reflashed, so
Workload OTA is unavailable on them until then (the web flasher always erases);
the Agent `partition_layout` identifier (`embewi-ab-v1`) must not change, so a
Workload layout identifier would be separate. Slot sizes depend on the real
Workload size, which is unknown yet. S15 wires no ESP storage.
