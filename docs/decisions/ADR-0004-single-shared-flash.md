# ADR-0004 — One ESP physical flash owner

Status: Accepted.

## Decision
Instantiate physical ESP flash once and serialize shared users through SharedFlash.
Native multicore execution preserves multicore_auto_park during flash operations.

## Consequences
NVS, OTA and Workload storage cannot create competing physical flash owners.
