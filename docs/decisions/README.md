# Architecture decision records

Accepted decisions are stable context for humans and agents. A later decision that
replaces one of these records MUST explicitly supersede it.

- [ADR-0001](ADR-0001-agent-workload-model.md) — Agent/Workload model
- [ADR-0002](ADR-0002-dual-ota.md) — independent Agent and Workload OTA
- [ADR-0003](ADR-0003-local-slot-authority.md) — local physical slot authority
- [ADR-0004](ADR-0004-single-shared-flash.md) — single ESP flash owner
- [ADR-0005](ADR-0005-native-workloads.md) — native target-specific Workloads
- [ADR-0006](ADR-0006-reject-wasm.md) — Wasm/wasmi rejected
- [ADR-0007](ADR-0007-explicit-workload-abi.md) — no Rust ABI across boundary
- [ADR-0008](ADR-0008-iwni-v1.md) — IWNI native image format
- [ADR-0009](ADR-0009-trusted-native-workload.md) — trusted, non-isolated model
- [ADR-0010](ADR-0010-runtime-roles.md) — NativeRuntime vs ProbeRuntime
- [ADR-0011](ADR-0011-runtime-api.md) — RuntimeApi compatibility
- [ADR-0012](ADR-0012-boot-guard.md) — Workload boot guard
- [ADR-0013](ADR-0013-single-source-documentation.md) — README canonical, AGENTS/site generated
- [ADR-0014](ADR-0014-product-composition.md) — portable product policy and per-target composition
- [ADR-0015](ADR-0015-board-entry.md) — proposed Board contract and conditional framework-owned entry; partially supersedes ADR-0014 when delivered
- [ADR-0017](ADR-0017-wifi-access-point-port.md) — proposed `WifiAccessPoint` port; mode changes restart the radio and the station reconnects
