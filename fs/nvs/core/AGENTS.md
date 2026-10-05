# Agent Context — iobewi-nvs-core

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-nvs-core`
- Path: `fs/nvs/core`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Host-testable ConfigSpace NVS record framing, key validation and entry accounting.

## Owns

Encode/decode CSM1 records and translate payload budgets into reserved NVS entry counts.

## Does not own

Physical flash access, locking, generation advancement, ConfigBackend implementation and configuration policy.

## Architecture position

Portable helper below the ESP ConfigSpace backend; it has no fs/config dependency and accepts plain byte budgets.

## Public contracts

`encode_record`, `decode_record`, `valid_space_name`, `entries_for_blob`, `reservation_units` and `capacity_units`; constants define a 13-byte CSM1 header and 15-byte maximum NVS key.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

`cargo test -p iobewi-nvs-core` checks golden bytes, corruption, key rules and accounting overflow.

## Known limitations

Uses alloc for encoding. Keys must be nonempty ASCII without NUL and at most 15 bytes. Decoder rejects short headers, wrong magic and unknown flag bits. Reservations include two versions of the largest record: 2 × (ceil((13 + budget)/32) + ceil((13 + budget)/4000) + 1); capacity keeps one page in reserve. This is entry accounting, not a guarantee against all storage failures.

## Related components

`fs/nvs/config-esp32` consumes framing/accounting; `fs/nvs/esp32` owns the borrowed NVS view.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
