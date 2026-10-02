# Workload OTA over HTTPS (S17)

The Workload update level of `docs/dual-ota.md`, reachable through the Agent's existing
HTTPS server. **Selection and staging only**: there is still no Workload supervisor,
loader or runtime, so `activate` is refused and nothing is ever started.

Namespace: `/v1alpha1/workload/ota/*` (the Agent's `/v1alpha1/ota/*` is unchanged and
keeps meaning `UpdateTarget::Agent`; a separate namespace was preferred to an
ambiguous `target` field in the Agent routes). HTTPS only (port 80 stays closed),
same `Authorization: Bearer <admin token>` as every other route.

## Routes

| method | path | request | success | errors |
|---|---|---|---|---|
| GET | `/status` | – | 200 capability + OTM2 state (200 even when unsupported) | 401 |
| POST | `/prepare` | JSON: `artifact_id`, `version`, `size`, `digest` (`sha256:<64 hex>`), `required_runtime_api` `{major,minor}` | 200 `{"accepted":true,"max_artifact_size":N}` | 400 malformed/`bad_digest`, 401, 409 busy/unsupported, 413 too large, 500 |
| PUT | `/write` | headers `X-Embewi-Digest`, `Content-Range: bytes s-e/total`; body = chunk | 200 `{"status":"partial","written":N}` / final 200 `{"status":"staged","written":N,"digest":...}` | 400, 401, 409 not_prepared/session_mismatch, 416 range_mismatch (+`written`), 422 digest_mismatch, 500 |
| POST | `/activate` | JSON: `digest` of the staged candidate | (tests only) 200 `{"status":"activated"}` | 400, 401, 409 not_staged/incompatible_runtime_api/candidate_mismatch/unsupported, **501 `supervisor_unavailable`** |

Core never sends or receives a slot, an offset or a partition name. `status` may carry
`diagnostic.active_slot/candidate_slot`; nothing needs them.

`status` fields: `supported`, `reason` (when unsupported, e.g. `MissingMeta`), `state`
(`none|empty|valid|staged|activating|pending_confirmation|rolling_back|corrupted`),
`active` / `candidate` / `previous` (`id`, `version`, `digest`, `size`,
`required_runtime_api`), `max_artifact_size`, `runtime_api_provided`,
`write_in_progress`, `prepared`.

## HTTP -> OTM2 mapping

| HTTP | service | OTM2 / state |
|---|---|---|
| `POST /prepare` | `prepare` (inactive slot chosen locally by OTM2; refused when `Activating`/`PendingConfirmation`/`RollingBack`; supersedes a `Staged` candidate) | no write; a RAM session remembers the prepared artifact |
| `PUT /write` first chunk (`start = 0`) | `begin` (digest/total must be the prepared ones) | no write |
| `PUT /write` next chunks | `chunk`: erase-ahead 64 KiB, SHA-256 incremental, one 4 KiB unit durable at a time | none |
| `PUT /write` last chunk | `finish`: size + SHA-256 verified, then `commit_staged` | **Empty/Valid/Staged -> Staged** (only on a verified digest) |
| `POST /activate` | `check_activation` (supported, `Staged`, candidate digest, runtime API) then the activation port | production: **no OTM2 write, state stays `Staged`**, 501 |
| `GET /status` | `status` | read only |

A reboot loses the RAM session (not the OTM2 state): a client simply re-prepares.
A wrong digest is never staged and leaves the active Workload untouched.

## Activation is fail-closed

`NoSupervisor` (the production S17 value) runs every check -- supported, state
`Staged`, the named candidate, runtime API -- and then answers `501
supervisor_unavailable`. It never persists `Activating`, `PendingConfirmation` or
`Valid` and never calls a supervisor: OTM2 states stay the real state of the
Workload, not what the API would like to have done. Host tests plug a fake
supervisor through the same port. No Workload route reboots the Agent or touches
`otadata`, the bootloader or any Agent slot.

## Compatibility policy

`prepare`/`write` accept an artifact whose `required_runtime_api` is newer than the
Agent's (so a Workload can be preloaded before an Agent update); `activate` is where
the gate is: `agent.satisfies(required)` (same major, minor >= required) or `409
incompatible_runtime_api` with both versions, and the candidate stays `Staged`.
The Agent's provided API is one constant (`RUNTIME_API`, initially `1.0`) in the
Agent's `workload` module.

## Error table

| condition | status | `error` |
|---|---|---|
| no / wrong Bearer | 401 | `unauthorized` |
| malformed body/headers | 400 | `bad_request`, `bad_digest`, `bad_content_range`, `content_length_mismatch`, `empty_body`, `empty_artifact`, `bad_field`, `missing_digest` |
| Workload storage unsupported | 409 | `workload_storage_unsupported` + `reason` |
| prepare during Activating/PendingConfirmation/RollingBack | 409 | `workload_busy` + `state` |
| activate when not `Staged` | 409 | `not_staged` + `state` |
| write without a matching prepare | 409 | `not_prepared` / `session_mismatch` |
| activate names another candidate | 409 | `candidate_mismatch` |
| runtime API not met | 409 | `incompatible_runtime_api` + `required`/`provided` |
| artifact larger than a slot | 413 | `size_too_large` + `max` |
| bad `Content-Range` offset (gap/overlap) | 416 | `range_mismatch` + `written` |
| SHA-256 mismatch | 422 | `digest_mismatch` + `computed` |
| no supervisor | 501 | `supervisor_unavailable` |
| flash/metadata failure | 500 | `storage_failure`, `incomplete`, `otm2_corrupted` |

Reused from the Agent OTA where it expresses the same problem (413 size, 416 range,
409 busy/conflict/not staged, 400 malformed, 500 storage). One deliberate
difference: a digest mismatch is `422` here, whereas the Agent route's historical
`200 {"status":"digest_mismatch"}` is left untouched.

## Layering

```text
embewi-agent (composition)   Authorize = agent Bearer, NoSupervisor, EspFlashAccess
        |
workload/http                routes, parsing, error table      (no ESP, no flash)
        |
workload/update::service     capability, prepare/begin/chunk/finish, activation gate
        |
workload/update::flash       one lock per operation (FlashAccess), OTM2, SlotWriter
        |
workload/esp32               partition lookup by label (iobewi-esp-partitions),
                             SharedFlash lock -> the one FlashStorage
```

Lock discipline: handlers hold no flash lock across HTTP parsing, auth, TLS or
ConfigSpace; each storage step takes the single `SharedFlash` for one closure.

## Not in S17

Supervisor/loader/Wasm, OTM2 changes, partition changes, new endpoints on the
Agent's `/ota/*`, `embewi-core` changes.
