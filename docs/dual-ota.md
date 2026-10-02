# Dual-OTA model (S14)

Status: **architecture decision + portable model** (`firmware/model`, crate
`iobewi-update-model`). No active code path changes: the Agent OTA in production
(OTM1, EWBT, `iobewi-ota`) is untouched.

## 1. Terminology

| Term | Meaning |
|---|---|
| **Agent** | `embewi-agent`: the resident runner / workload supervisor of the device. |
| **Workload** | The supervised application / Pod: business logic. |

The Agent is **not** an operating-system kernel: no syscalls, no user/kernel
modes, no MMU/MPU, no process model, no OS scheduler are implied or planned by
this model. The words *kernel* / *userspace* / *kernel ABI* are not used in code
or docs; the compatibility contract is the **runtime API** the Agent provides.

## 2. Two update levels, one order source

```text
                embewi-core
                     │            (desired state: artifact, digest, size,
           ┌─────────┴─────────┐   version, target = Agent | Workload;
           │                   │   never a slot)
       OTA Agent          OTA Workload
           │                   │
           ▼                   ▼
 inactive Agent slot    inactive Workload slot
           │                   │
        reboot              activate
           │                   │
           ▼                   ▼
      bootloader            supervisor
```

Boot chain: `ROM → bootloader → Agent A/B → workload supervisor → Workload A/B`.

| Level | Written by | Activation authority | Reboot of the Agent | Confirmation | Rollback |
|---|---|---|---|---|---|
| Agent | the running Agent (inactive Agent slot) | **bootloader** + Agent OTA metadata | required | after reboot + Agent self-check (+ can it still run the active Workload) | bootloader returns to the previous Agent |
| Workload | the running Agent (inactive Workload slot) | **the Agent** (supervisor) | not required | after Workload health/liveness | Agent restarts the previous Workload |

The bootloader never selects a Workload. Core never selects A or B: it names a
logical target and an artifact; the device picks its own inactive slot (the
slot may be reported as a diagnostic, never used to express desired state).

Agent and Workload are **not** one atomic release: `Agent A + Workload A` →
`Agent A + Workload B` (no Agent update) and `Agent B + Workload B` (after an
Agent update) are both normal. Core may order them in either order.

## 3. What is shared and what is not

```text
 embewi-core
      │
      ▼
 control-plane translation  (Pod desired state → UpdateRequest)   [agent / control plane]
      │
      ▼
 generic update engine      (prepare, streaming write, digest, size,   [firmware/update:
      │                      resume/resync, supersession, reconcile]     iobewi-ota]
   ┌──┴─────────┐
   ▼            ▼
 Agent policy  Workload policy                                    [firmware/model:
   │            │                                                  AgentOta / WorkloadOta]
   ▼            ▼
 bootloader   supervisor / runtime                                [platform + agent]
```

* **Shared engine** (`iobewi-ota`): transaction record
  (`TransactionRecord<TxId, ArtifactId, Target>` with an opaque `Target`),
  streaming write with digest, resume, supersession, reconcile table. Imposes no
  reboot, no bootloader, no loader, no health policy.
* **Agent policy** (`AgentOta`): inactive-slot staging, boot activation through a
  `BootAuthority`, `PendingConfirmation`, confirm/rollback, watchdog self-check
  (existing machinery in `iobewi-ota::service::boot`, EWBT in `firmware/boot`).
* **Workload policy** (`WorkloadOta`): inactive-slot staging, compatibility
  check, activation through a `WorkloadSupervisor`, health, confirm/rollback.
* Several small contracts, no giant trait: `BootAuthority`,
  `WorkloadSupervisor`, `AbSlots<T>` (A/B slot set, **no policy**),
  `UpdateRequest` / `ArtifactDescriptor` / `Compatibility`, `RuntimeApi`.

## 4. State machines

Agent OTA (existing, kept; names from `iobewi-ota`):

| State | Event | Next | Authority |
|---|---|---|---|
| none | prepare + write + digest ok | `Staged` | Agent |
| `Staged` | activate | `Activating` (metadata committed, then boot scheduled) | Agent → bootloader |
| `Activating` | reboot, bootloader picks the new slot | `PendingConfirmation` (EWBT Pending) | bootloader |
| `PendingConfirmation` | self-check ok (and active Workload runnable) | confirmed → `Valid` | Agent |
| `PendingConfirmation` | self-check fails / watchdog / incompatible Workload | rollback to previous Agent | bootloader |
| `Activating` | power cut | `reconcile` → await / finish / clear | engine |

Workload OTA (target, modelled in `WorkloadOta`; persistence deferred):

| State | Event | Next | Authority |
|---|---|---|---|
| `Absent` | – | – | – |
| `Staged` | write + digest ok | `Staged` (inactive slot) | Agent |
| `Staged` | activate, Workload API requirement ≤ Agent API | `PendingConfirmation` (running) | Agent (supervisor) |
| `Staged` | activate, requirement not met | refused, Workload unchanged | Agent |
| `PendingConfirmation` | health ok | `Valid` | Agent |
| `PendingConfirmation` | health fails | `RolledBack` → previous Workload restarted | Agent |

The new Workload states are only those the Agent-side policy needs; no state
already provided by the engine (`Staged`/`Activating`, `PendingConfirmation`,
`Confirmed`) is duplicated in a different spelling.

## 5. Responsibilities

| Actor | Owns |
|---|---|
| `embewi-core` | desired state: `agent_desired`, `workload_desired`; orders the two OTAs separately; observes `agent` and `workload` status separately |
| `embewi-agent` | hardware access, drivers, network, security, configuration, **Agent OTA** (prepares), **Workload OTA** (writes + activates), workload supervision, health, resource control, runtime integration, reporting to Core |
| bootloader | physical choice of the Agent slot (EWBT Pending/Valid/Aborted); never the Workload |
| workload supervisor / runtime | start/stop/switch/restore of the Workload on the Agent's order |
| Workload | business behaviour only; does **not** own Wi-Fi provisioning, device TLS identity, Agent OTA, boot metadata or the device control plane |

Already implemented: hardware, drivers, network, security, configuration,
Agent OTA, reporting. Future capability: Workload OTA, workload supervision,
health, resource control, runtime integration. The runtime itself (Wasm, native
loader, MPU, isolation, preemptive scheduler, syscalls, ABI) is **out of scope**.

## 6. Core order → engine request (no slot exposed)

```rust
// Translation lives in the agent / control-plane layer, never in the engine.
let req = UpdateRequest::workload(
    ArtifactDescriptor { id, version, digest, size },
    RuntimeApi::new(1, 3),           // runtime API the Workload requires
);
// UpdateRequest::agent(artifact, provides) for an Agent artifact.
```

`UpdateRequest` has no slot field and fixes target and compatibility together
(`req.target()` is derived from the compatibility variant, so they cannot
disagree). The engine and the policies never see a Pod, a Kubernetes type or an
`embewi-core` API type.

Core-side status (proposal, format not frozen):

```text
agent:    { current, desired, state }
workload: { current, desired, state }
diagnostic only: active_slot per level
```

## 7. Compatibility

* The Agent provides a `RuntimeApi { major, minor }`; a Workload requires one.
  `provided.satisfies(required)` ⇔ same major and `provided.minor ≥ required.minor`.
* **Workload activation** is refused before the Workload becomes active when the
  running Agent does not satisfy it (the supervisor is never called).
* **Agent update with an existing Workload**: after the reboot, the new Agent is
  confirmed only if its self-check passes **and** it satisfies the active
  Workload; otherwise it is rolled back (explicit default policy, see
  `Reason::IncompatibleWorkload`). A different policy (confirm degraded) would be
  an explicit, separate decision.

## 8. Recovery

| Failure | Result |
|---|---|
| Agent update fails | Workload unchanged (tested) |
| Workload update fails | Agent unchanged (tested) |
| New Agent boots but cannot run the active Workload | Agent not confirmed → previous Agent (tested) |
| Power cut during Agent activation | existing `reconcile` outcome, unchanged |

## 9. OTM1 / OTM2

* **OTM1** (`b"OTM1"`, `iobewi-ota::metadata`): the persistent record of the
  *Agent* OTA (stage, slot, digest, deployment_id, size). It is the reference for
  `UpdateTarget::Agent`; format unchanged, deployed devices stay readable.
* **OTM2** — compared options:

| Criterion | A: OTM1 = Agent, OTM2 = Workload | B: OTM2 = generic, `target = Agent \| Workload` |
|---|---|---|
| Compatibility | OTM1 untouched | needs a migration of deployed OTM1 records |
| Migration risk | none for the bootloader-driven path | Agent record rewritten on live devices |
| Atomicity | independent transactions (matches "not one release") | invites one shared transaction (explicitly not wanted) |
| Complexity | two small schemas | one larger schema + discriminant + migration |
| Persistence | separate ConfigSpace namespaces | one namespace |
| Evolvability | a later generic format is a clean OTM3 | locks the generic shape early |

**Decision: Option A.** OTM2 is the persistent record of the *Workload* OTA,
an independent transaction, **not** an Agent+Workload release. No binary OTM2 is
implemented in S14 (S15 implements it as a portable engine + codec, see `docs/otm2.md`); before any binding to a platform, fix: semantics, versioning, integrity,
supersession, migration/compat. The HTTP surface can gain an optional `target`
(default `Agent`, so OTM1 clients keep working) or a separate `/workload/ota/*`
prefix; the existing `/ota/prepare|write|activate` endpoints are not modified.

## 10. Source tree

* **Option 1** `update/{core,agent,workload}`: symmetric, but promotes
  "update" to a root and moves a crate that is already used by the Agent.
* **Option 2** `firmware/update` + `workload/update`: Agent-side stays where it
  is; Workload lifecycle gets its own root because it is *not* firmware and
  its authority is the supervisor, not the bootloader.

Recommendation: **Option 2**, taken lazily — keep `firmware/update` (engine and
Agent policy) and `firmware/model` (shared vocabulary + the two policies as an
executable specification) as they are; create a `workload/` root only when real
Workload code (supervisor integration, persistence, loader) exists. No move in S14. (S15 created `workload/update` for the real Workload OTA code: OTM2 codec, double-copy store and state machine — see `docs/otm2.md`.)

## 11. Portability

The model has no dependency at all (`no_std` + `alloc`): nothing from ESP,
`esp-hal`, `embassy-net`, HTTP or Kubernetes. Another platform (RP2350,
Teensy / i.MX RT) provides its own `BootAuthority` and storage, and reuses the
same concepts unchanged. Workload storage may later be a flash partition, a
filesystem, an external flash or a RAM image without changing the Core contract.
