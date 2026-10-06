# Agent Context — iobewi-log-config

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-log-config`
- Path: `log/config`
- Layer: `portable-service`
- Status: `implemented`

## Role

Portable ConfigSpace binding for a bounded, persistent LogPolicy.

## Owns

Own the version-one log policy YAML schema, validate the complete configuration, load and apply persisted policy, and persist replacements before applying runtime filtering.

## Does not own

Physical persistence, NVS/flash/ESP, console hardware, transport authorization, automatic change detection and global logger installation.

## Architecture position

Composition layer above log/core and fs/config. The product claims the dedicated `log` ConfigSpace using `Budget::new(POLICY_BYTES_MAX)` (1024 bytes) and transfers its unique handle to `LogConfig`. INV-001 is preserved: only portable contracts are dependencies.

## Public contracts

`LogConfig::new(space)` takes ownership of the unique capability. `reload(&mut self)` loads, validates and applies a complete policy, returning `Some(generation)` or `None` for missing data. Missing data, schema errors and backend failures leave the current policy untouched. `replace(&mut self, policy)` validates/encodes, commits, then applies; a failed commit never changes runtime filtering. Exclusive mutable access serializes these operations. Cancellation/power loss after commit and before apply is reconciled by the next reload.

`encode` / `decode` define the following restricted YAML v1, inside the dedicated space (not a merged product configuration):

```yaml
log:
  default_level: off
  targets:
    streambewi: debug
    embassy_usb: trace
    streambewi::usb: info
```

Keys and levels are lowercase, ordered as shown, with two/four-space indentation. `targets` may be absent or empty. Level values are off/error/warn/info/debug/trace. Plain target keys start with an ASCII alphanumeric character or underscore and contain ASCII alphanumeric characters or `_:-./`; a final colon is rejected. Rust `::` namespaces are supported. Blank lines and CRLF are accepted. Quotes, comments, aliases, tags, flow maps, arbitrary YAML syntax, unknown keys and duplicate prefixes are rejected. The parser is allocation-free and bounded by 1024 input bytes, eight prefixes and 64 bytes per prefix. General runtime policies can contain other UTF-8 prefixes, but encoding rejects those not representable by this persisted schema.

## Invariants

- `INV-001`

## Modification context

### Lifecycle

Install log/core first for early logs. Claim ConfigSpace during deterministic boot configuration, construct LogConfig, then call reload. No stored policy preserves the legacy fallback; invalid data reports an error so the product can surface it without losing fallback logs. To update diagnostics, call replace through the application; successful return means persistence and runtime apply completed. There is no polling task: explicit reload handles other authorized configuration update paths. The fallback is not automatically written into ConfigSpace. Neither operation changes log-stream authorization.

## Required validation

`cargo test -p iobewi-log-config -p iobewi-config-space` covers round trips/all levels, maximum schema capacity, invalid/duplicate/unknown fields, missing values, persisted reload, failed commits and corrupt data without changing runtime state. BG-STORAGE applies to the ConfigSpace boundary; host tests validate the portable binding, while physical storage qualification remains with the unchanged backend. BG-ESP-S3 applies when a downstream product wires the binding and transport on hardware.

## Known limitations

Restricted YAML v1 is deliberately not a general YAML parser. No schema migration or automatic notification watcher is provided. Products must reserve the claim and invoke reload/replace; Board and entry are unchanged. Backend atomicity and generations follow ConfigSpace. Clearing a value does not itself reset the runtime policy; explicitly apply the fallback if desired.

## Related components

`log/core`, `fs/config`, `log/stream`.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
