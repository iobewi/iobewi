# ADR-0002 — Independent Agent and Workload OTA

Status: Accepted.

## Decision
Agent OTA and Workload OTA are independent transactions, persistent records and
activation policies. Agent uses OTM1; Workload uses OTM2. They are not an atomic release
pair.

## Consequences
Either side can update/rollback without rewriting the other's lifecycle.
