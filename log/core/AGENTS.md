# Agent Context — iobewi-log

<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->

- Package: `iobewi-log`
- Path: `log/core`
- Layer: `portable-contract`
- Status: `implemented`

## Role

Local, process-wide log capture with a bounded runtime policy, without a network dependency.

## Owns

Install the global logger once, filter synchronously, invoke the supplied console callback for accepted records, and capture level/target/message in a critical-section protected FIFO. `Off` rejects before console output, formatting or capture. No async loop is required.

## Does not own

Console hardware, persistence, network activation/authorization and delivery. Capture policy never grants permission to stream logs.

## Architecture position

Portable capture layer consumed by log/stream. The target supplies the console callback and application prefix; log/config binds ConfigSpace to this policy without introducing platform dependencies (INV-001).

## Public contracts

- `install(print, application_target)` once during single-threaded startup. Without an explicit policy, raw prefixes `application_target` and `iobewi_log` accept Info and above, other targets Warn and above, exactly as before. Legacy empty/long application prefixes remain supported for filtering.
- `LogPolicy::new(default_level)` supports Off, Error, Warn, Info, Debug and Trace. `add_target(prefix, level)` adds at most eight unique, nonempty UTF-8 prefixes, each at most 64 bytes. Longest matching raw prefix wins, independently of rule order; otherwise default applies. This preserves historical prefix semantics (including `iobewi_log_stream`). `targets()` exposes immutable rules; malformed/duplicate/overflow additions return `PolicyError` without changing the policy.
- `default_policy(application_target)` constructs the bounded equivalent of the legacy fallback, returning an error for an unrepresentable empty/long prefix. Installation itself does not have this restriction.
- `apply_policy(policy)` atomically replaces runtime filtering without reinstalling the logger. Facade maximum remains Trace so concurrent updates cannot desynchronize policy and max level. In-flight records that passed `enabled()` before replacement may complete; queued records are retained. Call `discard()` explicitly if required.
- `pop_record()` removes a `CapturedRecord` preserving `level`, `target` and `message`. `pop_line()` remains a compatibility API returning only its message; both consume the same FIFO.
- `LINE_MAX = 160`, `TARGET_MAX = 64`, `RING_CAPACITY = 24`. Oversized messages/targets and incoming records at a full ring are dropped, never truncated or used to evict older entries. Console callback still precedes capture attempts.
- `LogMetadata` supplies delivery identity/time metadata; it is separate from captured origin metadata.

## Invariants

- `INV-001`

## Modification context

See the canonical README and implementation.

## Required validation

`cargo test -p iobewi-log` covers legacy filtering, Off before formatting/console, runtime facade updates, prefix precedence, fallback, rule bounds, original metadata, ring overflow, oversized records, boundary-sized records and discard. `cargo check --workspace --all-features` and `--no-default-features` preserve portability. BG-ESP-S3 is the downstream hardware gate when qualifying changed console/WebSocket behavior; host tests do not declare it passed.

## Known limitations

Installation is not repeatable. Runtime filtering cannot restore levels removed by product `release_max_level_*` features. Rules are raw prefixes, not glob/regex expressions. Storage is best effort; full rings retain oldest records. `Off` does not erase records already queued or stop a transport. No dedicated worker, persistence or notification watcher exists in core.

## Related components

`log/config` owns the portable persisted schema/binding; `log/stream` consumes structured records; `drivers/console/esp32` supplies the console callback.

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
