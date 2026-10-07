# Agent Context — iobewi-wifi-manager

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-wifi-manager`
- Path: `net/wifi/manager`
- Layer: `portable-service`
- Status: `implemented`

## Role

Wi-Fi manager: durable credentials (WFC1), reconnection, provisioning and reprovisioning over a WifiTransport

## Owns

- Own durable station credentials (WFC1), the reconnection loop with bounded backoff, and provisioning with rollback (connect first, commit only on success, restore the previous network on failure).
- Delegate the optional soft access point to the transport (`start_access_point`, `stop_access_point`, `is_access_point_active`, `access_point_handle`), available when the transport also implements `WifiAccessPoint`.
- Preserve the `portable-service` boundary.

## Does not own

- Does not access a platform HAL directly.
- Does not own platform-specific device mechanics.
- Does not decide when the access point opens, for how long, or which SSID/passphrase it uses: that is product policy. The access point lives in RAM and the manager never writes it to config-space.

## Architecture position

Path: `net/wifi/manager`. Layer: **portable-service**.

Local IOBEWI dependencies declared by Cargo:
- `../../../fs/config`
- `../core`

## Public contracts

Exported Rust items are the code-level API authority. Package features and dependency declarations remain canonical in `Cargo.toml`.

**Access point sequence.** The access point methods borrow the manager exclusively, like `provision`, so they never overlap `maintain`. Starting or stopping the access point may restart the radio and drop the station link, so: drop the `maintain` future, change the access point, call `maintain` again; it reconnects from the saved credentials.

Provisioning a device with no saved credentials (`MaintainError::NotProvisioned`): start the access point, serve the product's provisioning page on `access_point_handle()`, call `provision` (it connects first and commits only on success, with the access point still up so the response reaches the client), stop the access point, call `maintain`. On a failed `provision` the access point stays up for a retry.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

- `cargo test -p iobewi-wifi-manager`: includes access point delegation, no config-space write on start/stop, a failed start, provisioning through an active access point, and recovery of a radio restart by `maintain`.
- No hardware baseline gate is declared here; radio behavior is validated at the platform adapter.

## Known limitations

No additional crate-specific limitation is recorded beyond the repository current-state and open-debt documents.

## Related components

- Root `ARCHITECTURE.md` and `INVARIANTS.md`.
- `Cargo.toml` for package features/dependency facts.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
