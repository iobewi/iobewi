# Fresh-agent documentation acceptance test

Purpose: test the repository knowledge system rather than the memory of a particular
agent.

## Protocol

1. Use an agent with no prior IOBEWI conversation context.
2. Give it the repository and instruct it to begin at root `AGENTS.md`.
3. Ask it to explain the current architecture, important invariants and validation model.
4. Give it a future change goal without a custom historical summary.
5. Record wrong architecture assumptions, unnecessary archaeology, missed invariants,
   wrong gate selection and ambiguous ownership.
6. Treat those failures as documentation gaps and correct the canonical README/global
   source before retrying.

## Initial dry-run question

Without implementing code:

> Where would you modify IOBEWI to add a future I2C Workload capability, which dependency
> direction must be preserved, and which invariants/gates would you inspect?

Expected reasoning includes a portable Workload capability/service boundary, a separate
platform backend, no direct `esp-hal` dependency from business Workload code, and
selection of the relevant invariants/gates rather than inventing a second runtime.
