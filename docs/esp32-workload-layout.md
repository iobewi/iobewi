# ESP32 flash layout for the Workload OTA (S16)

Scope: the **physical** storage of the Workload OTA (`docs/dual-ota.md`,
`docs/otm2.md`): OTM2 metadata plus Workload slots A/B. Selection state only: no
loader, no runtime, no endpoint. Agent OTA (OTM1, `ota_0`/`ota_1`/`otadata`) is
untouched and independent.

## 1. Layout (ESP32-S3, 16 MiB — `embewi-agent/partitions.csv`)

```text
0x000000  bootloader (0x0..0x8000) / partition table (0x8000)  [unchanged]
0x009000  nvs        0x006000   24 KiB                          [unchanged]
0x00f000  otadata    0x002000    8 KiB   (EWBT, OTM1 side)      [unchanged]
0x011000  phy_init   0x001000    4 KiB                          [unchanged]
0x020000  ota_0      0x180000  1.5 MiB   Agent slot A           [unchanged]
0x1a0000  ota_1      0x180000  1.5 MiB   Agent slot B           [unchanged]
0x320000  wl_meta    0x002000    8 KiB   OTM2: copy A | copy B  [new]
0x330000  workload_a 0x5E0000  5.875 MiB Workload slot A        [new]
0x910000  workload_b 0x5E0000  5.875 MiB Workload slot B        [new]
0xEF0000  (free)     0x110000  1.06 MiB  reserved for future system partitions
0x1000000 end of flash
```

| name | offset | size | type/subtype | owner | changed |
|---|---|---|---|---|---|
| nvs | 0x9000 | 0x6000 | data/nvs | ConfigSpace | no |
| otadata | 0xf000 | 0x2000 | data/ota | bootloader (EWBT), OTM1 side | no |
| phy_init | 0x11000 | 0x1000 | data/phy | radio | no |
| ota_0 | 0x20000 | 0x180000 | app/ota_0 | Agent OTA | no |
| ota_1 | 0x1a0000 | 0x180000 | app/ota_1 | Agent OTA | no |
| wl_meta | 0x320000 | 0x2000 | data/undefined | Workload OTA (OTM2) | **new** |
| workload_a | 0x330000 | 0x5E0000 | data/undefined | Workload OTA | **new** |
| workload_b | 0x910000 | 0x5E0000 | data/undefined | Workload OTA | **new** |

* The five existing entries are byte-identical in the binary table (checked).
  Offsets/sizes of `ota_0`, `ota_1`, `otadata` are a blocking invariant.
* **Subtype `undefined`, found by name.** The Agent's table parser
  (`esp-bootloader-esp-idf`) `unwrap`s the subtype of every entry it inspects, so a
  custom subtype (0x40..) would make the Agent *panic* while looking up
  `ota_0`/`nvs`. `undefined` (0x06) is the one unnamed data subtype it accepts; the
  three partitions are told apart by label. The EWBT bootloader ignores entries it
  does not know. Rust code carries **no offset**: the table is the only source of
  truth (`partitions.csv` is checked by `scripts/check-partitions.py`).
* **wl_meta = two erase units**, copy A in the first 4 KiB sector, copy B in the
  second, so one sector erase can never destroy both OTM2 copies.
* **Alignment audited**: table entries 4 KiB; app partitions 64 KiB; NOR erase unit
  4 KiB, write unit 4 bytes; erase-ahead uses 64 KiB block erase, so the Workload slots
  start on 64 KiB boundaries (`0x330000`, `0x910000`). Flash encryption and secure boot
  are **not** enabled (no partition carries the `encrypted` flag); if flash
  encryption is enabled later the Workload partitions must be marked and written
  through the encrypted path; a Workload signature distinct from the SHA-256 is a
  future concern, not implemented.

### Slot sizing (S3)

Agent first, then metadata, then the Workload slots, then an explicit reserve:

| step | decision |
|---|---|
| system + Agent A/B | unchanged, ends at `0x320000` |
| OTM2 | 8 KiB |
| Workload slots | `(0xEF0000 − 0x330000) / 2 = 0x5E0000` each, 64 KiB multiples |
| reserve | 1.06 MiB (6.6 %) kept free: new partitions cannot be added after the Workload slots without moving them |

* **Maximum Workload artifact** = one slot = 6,160,384 B (5.875 MiB). Larger artifacts
  are refused before any write (`TooLarge`).
* The placeholder "4 MiB each" of S15 is replaced: nothing justifies capping the
  principal consumer of application space at 4 MiB when 12.9 MiB are free.

## 2. Capability

| device | Agent OTA | Workload OTA |
|---|---|---|
| ESP32-S3 16 MiB, S16 table | supported | **supported** (2 × 5.875 MiB) |
| ESP32-S3, table from S15 or older | supported | **unsupported** (`MissingMeta`) |
| ESP32-C3 4 MiB, reference layout below | supported | supported, 2 × 444 KiB (**no C3 Agent exists yet; layout not instantiated**) |
| any table without the three partitions or with an invalid/overlapping set | supported | unsupported — never "free flash" |

The two are distinct capabilities. At boot the Agent probes the table
(`iobewi_esp_workload::probe`): `Supported(layout)` or `Unsupported(reason)`; it
logs it and keeps `Option<EspWorkloadStorage>`. A Workload partition that overlaps
any other partition makes the whole device unsupported.

## 3. Flash capacity planning

### Agent sizes (the `.bin` actually written into `ota_x`, not the ELF)

| build | `.bin` bytes |
|---|---|
| S1 … S13 (13 recipe builds) | 1,086,272 … 1,089,968 (±0.35 % over the whole migration) |
| S13 | 1,089,776 |
| **S16 plain** | **1,097,328** (+7.5 KB: probe, OTM2 recovery, Debug) |
| S16 `workload-selftest` (test image only) | 1,116,144 |
| ESP32-C3 Agent | none exists (the Agent is S3-only); the S3 size is used as a conservative upper bound |

The Agent is a resident supervisor, not the application: it grew 0.7 % in 13
migration steps. It can still absorb system code (Workload OTA wiring, supervision,
runtime glue, protocol evolution) but not Pod business logic.

### Agent budget

`MAX_AGENT_IMAGE_SIZE` = the slot, **0x180000 = 1,572,864 B** (S3 and C3).
Growth margin today: 475,536 B (**43.3 %**). A minimal embedded interpreter/loader and
supervisor are expected in the 250–400 KiB range, which this margin accommodates.
Enforcement: `scripts/check-partitions.py … --agent-bin` (called by
`scripts/build-boot.sh`) fails the build with the real size, the maximum and the
overrun; `--min-agent-margin-percent` can add a margin floor.

### C3 4 MiB: three scenarios

Fixed: bootloader/table/nvs/otadata/phy to `0x20000`; `wl_meta` 8 KiB; Workload slots
share what is left up to `0x400000`; S3-size Agent as the bound (1,097,328 B).

| | Agent slot ×2 | Agent margin | wl_meta | Workload slot ×2 | free |
|---|---|---|---|---|---|
| **A** current Agent slots | 0x180000 (1.5 MiB) | 43.3 % | 0x2000 | **0x6F000 = 444 KiB** | 0 |
| B comfortable | 0x170000 | 37.4 % | 0x2000 | 0x7F000 = 508 KiB | 0 |
| C conservative | 0x150000 | 25.4 % | 0x2000 | 0x9F000 = 636 KiB | 0 |

`MIN_WORKLOAD_SLOT_SIZE` = **256 KiB**: a small Pod (a compact module/native image)
plus its metadata; below that a slot is a toy. All three scenarios fit two useful
Workload slots, so **Workload dual-slot is supported on C3**. **Selected: A**, because
(1) it keeps `ota_0`/`ota_1` unchanged on C3 as well (no Agent-layout migration), (2)
it keeps the highest Agent margin (43 %) for loader/supervisor growth, (3) 444 KiB
is 1.7× the minimum. B/C only buy +14 % / +43 % Workload space at the price of an Agent
layout change and a smaller margin (C's 25 % is too tight for runtime glue).
Reference table: `embewi-agent/partitions-esp32c3.csv` (workload slots at
`0x322000` and `0x391000`, ending exactly at `0x400000`); validated by CI, used by no
build because there is no C3 Agent. If a real C3 Agent turns out larger than the S3
bound, the table is revisited then (explicit layout change).

## 4. Existing devices

An Agent OTA writes only `ota_0`/`ota_1` (and `otadata`/NVS metadata); nothing in the
update path writes the partition table (`0x8000`) or the bootloader. Therefore:

| device | what it sees |
|---|---|
| flashed with the S15 (or older) table, then OTA to an S16 Agent | **partition table unchanged** → no `wl_meta`/`workload_*` → `Workload OTA = Unsupported (MissingMeta)`; the Agent works exactly as before. An Agent OTA never gives it Workload storage. |
| factory-flashed with the S16 image (web flasher erases everything) | new table, Workload supported |
| S16 table, then OTA to an older Agent | Agent works; Workload partitions are simply ignored |
| a **full reflash** | the only way for an old device to gain Workload storage |

The Agent layout identifier `embewi-ab-v1` describes only the Agent A/B pair and does
**not** change (an OTA between layouts is still accepted).

## 5. Backend (`workload/esp32`, `workload/update`)

```text
workload/update   portable engine + layout + NOR backend (embedded-storage only)
      ↓
workload/esp32    partition lookup by name, overlap check, SharedFlash lock
      ↓
drivers/flash/esp32 (SharedFlash)  →  the one FlashStorage
```

* **One lock**: every operation locks `SharedFlash` for one step and releases it
  (same contract as the Agent OTA adapter); no second mutex, no second
  `FlashStorage`. Digest read-back locks per 4 KiB so NVS/OTA metadata are not
  starved. `activate` calls the (synchronous) supervisor with the lock held: it
  cannot await flash, so it cannot deadlock.
* **Bounds**: every read/erase/write is checked against its target region; the
  backend only ever receives the three validated Workload regions, so `ota_0`,
  `ota_1`, `otadata`, `nvs`, `phy_init`, the table and the bootloader are unreachable.
  Host tests fill all of them with a sentinel and run full lifecycles (S3 and C3
  layouts): not one byte changes.
* **Atomic metadata**: double copy (OTM2), erase-then-program of the older copy.
  A fake flash with 4 KiB erase, bit-clearing writes and a cut injected at **every**
  erase/program of a full lifecycle never loses or invents a Workload and never
  damages the active slot.
* **Erase strategy**: erase ahead in 64 KiB blocks while writing (lock per block);
  whole-slot erase exists for maintenance only. No yield-point/watchdog change was
  needed (see measurements).
* **Security**: a slot is meaningful only if a valid OTM2 record references it and its
  SHA-256 matches; slot bytes alone never select anything.

## 6. Factory erase

Policy: erasing `wl_meta` is sufficient — the slots become unselectable
(`NoWorkload`) whatever bytes they still hold (tested). A factory reset therefore
does not have to erase ~12 MiB. The web flasher erases all flash anyway. Not
implemented as a separate in-place factory path (none exists today).

## 7. Endurance (order of magnitude)

Per Workload update: 4 OTM2 commits (stage, activate ×2, confirm), +2 on rollback,
each erasing one 4 KiB sector of the two alternating → ≈ 2–3 erases per metadata
sector per update; each Workload slot is erased once per two updates. With ≥ 10⁵ NOR
cycles per sector this is > 30,000 updates; no wear levelling needed.

## 8. Hardware measurements (S3, `workload-selftest` image)

See the S16 report (filled from the serial log of the hardware gate).
