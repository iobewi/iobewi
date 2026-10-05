# ADR-0012 — Workload boot guard

Status: Accepted.

## Decision
Track unclean boots in RTC memory. After three unclean starts, suppress Workload
auto-start so the Agent remains recoverable. Thirty seconds of continuous healthy
execution clears the counter; clean reboots are marked.

## Consequences
OTM2 stays Valid and is not overloaded with boot-loop policy. A power-cycle resets the
RTC guard and grants another attempt.
