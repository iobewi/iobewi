# Workload Supervisor (S18)

The Supervisor makes the Workload lifecycle *real*: a Workload is only called `Valid`
if it was actually started, observed healthy and confirmed. Vocabulary: **Agent** and
**Workload** only. There is no kernel, no ABI, no loader here.

## Layers

```
HTTP  /workload/ota/{status,prepare,write,activate,confirm,rollback}   (iobewi-workload-ota-http)
        │  ControlPort
        ▼
WorkloadSupervisor<A, R>          execution state + orchestration     (iobewi-workload-ota::supervisor)
   │  persists via                    │  drives
   ▼                                  ▼
WorkloadOtaService / OTM2         WorkloadRuntime (trait)  ◄── stable boundary
(persistent state, flash)            ├─ ProbeRuntime  (S18, feature `workload-supervisor-probe`, validation only)
                                     └─ future real runtime (not decided here)
```

* **OTM2 = persistent state, Supervisor = execution state.** The Supervisor never writes
  OTM2 itself: it calls the persisted steps (`begin_activation`, `complete_activation`,
  `begin_rollback`, `complete_rollback`) of the Workload OTA engine around each effect.
* The slot is never loaded into RAM; the runtime reads it through `ArtifactReader`
  (one flash lock per read, never held while the Workload runs).
* One Workload at a time. Old one stopped, then new one started; the active Workload keeps
  running while the next is only staged.

## Stable vs temporary

| Stable (survives the real runtime) | Temporary (S18 only) |
|---|---|
| OTM2 states and transitions | `ProbeRuntime` and its `S18PROBE` header |
| `WorkloadRuntime` trait: `start/stop/health/running` | Fault injection (feature-gated, off by default) |
| `Health { Healthy, Unhealthy, Unknown }` | Counter-based health |
| Boot reconciliation rules | |
| `RUNTIME_API` single source (`src/workload.rs` in the Agent) | |

The probe header is **not an ABI**. Production images (no `workload-supervisor-probe`)
have no supervisor: activate/confirm/rollback answer `501 supervisor_unavailable`.

## State machine (OTM2 × Supervisor)

| OTM2 state | Supervisor action | Result |
|---|---|---|
| Staged | `activate(digest)`: support, state, candidate/digest, RuntimeApi checked; slot digest re-verified; persist **Activating**; stop old; start new | start ok → **PendingConfirmation** |
| Activating | start fails | automatic rollback → **Valid(previous)** or **Empty** |
| PendingConfirmation | `confirm()` only if the candidate is *really running* and `Healthy` | **Valid** (previous kept as `previous_valid`) |
| PendingConfirmation | `rollback()` | **RollingBack** → stop candidate, start previous → **Valid(previous)** / **Empty** |
| RollingBack | rollback failure (restore fails) | stays **RollingBack**, never an invented Valid |
| Valid / Empty | upload allowed (supersession invariant of S17 kept) | |

Upload is refused (409 `transition_in_progress`) during Activating, PendingConfirmation, RollingBack.

## Boot reconciliation (`reconcile_boot`, offline: no Wi-Fi, no Core)

| OTM2 at boot | Action | Outcome |
|---|---|---|
| Empty | nothing | `Idle` |
| Valid | re-verify digest, start | `Started(side)` / `StartFailed` |
| Staged | nothing runs | `Idle` |
| Activating | crash during activation → **RollbackRequired** → rollback | `RolledBack` |
| PendingConfirmation | reboot before confirmation → **rollback** (policy) | `RolledBack` |
| RollingBack | resume the rollback | `RolledBack` / `RollbackFailed` |
| digest mismatch | candidate/slot discarded, never run | `Corrupted` |
| partitions absent | | `Unsupported` |

An exhaustive power-cut test (`after_a_power_cut_anywhere_boot_reconcile_leaves_a_consistent_running_state`)
cuts power at every flash erase/program of a full scenario and checks that boot reconcile always
leaves OTM2 and what is running consistent.

## HTTP

| Route | Success | Errors |
|---|---|---|
| `POST /activate {"digest"}` | 200 `pending_confirmation` | 409 not staged / incompatible_runtime_api / transition_in_progress, 422 candidate_corrupted, 500 `activation_failed` (`rolled_back:true`), 501 no supervisor |
| `POST /confirm` | 200 `valid` | 409 wrong state / `workload_not_running` / `workload_unhealthy` (+health) |
| `POST /rollback` | 200 `rolled_back` | 409 nothing to roll back, 500 `rollback_failed` |
| `GET /status` | adds `runtime:{supervised,running,health,artifact}` | |

## Agent OTA guard

A new Agent in `PendingVerify` is **rejected** (previous Agent restored) if its `RUNTIME_API`
cannot satisfy the active Workload's required API. It is checked in `main.rs` before
`confirm_pending`. Test-only feature `test-runtime-api-0-9` simulates an incompatible Agent.

## Probe faults (validation builds only)

`none, fail-start, freeze, health-fail, reset-on-start, reset-on-stop`, selected by the artifact
header (`scripts/test-workload-ota.sh mkprobe`).
