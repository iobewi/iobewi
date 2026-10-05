# IOBEWI agent entry point

This file contains repository-wide rules for coding agents. Crate-local `AGENTS.md`
files are generated from the crate's canonical `README.md`; never edit a generated
crate `AGENTS.md` directly.

## Read order

Before modifying code:

1. Read `ARCHITECTURE.md`.
2. Read `INVARIANTS.md`.
3. For external product integration, read `docs/product-integration.md` and
   `docs/decisions/ADR-0014-product-composition.md`. Read `docs/contracts/README.md`
   and every applicable capability contract before writing assembly code.
4. Read the nearest generated crate `AGENTS.md`.
5. Read the crate `README.md` when rationale or details are needed.
6. Read every referenced contract, ADR and validation gate.
7. Inspect the implementation and tests.

## Sources of truth

- Code and executable tests describe the implemented behaviour.
- `INVARIANTS.md` is normative for repository-wide invariants.
- `docs/contracts/` and the canonical contract documents referenced there are normative for persisted, binary and network contracts.
- ADRs in `docs/decisions/` record accepted architectural decisions.
- A crate `README.md` is the canonical local semantic documentation for that crate.
- A crate `AGENTS.md` is a generated operational projection of its `README.md`.
- `Cargo.toml` is canonical for package identity, features and dependency declarations.

If these sources appear to contradict each other, do not invent a reconciliation. Report
the contradiction and use code/tests to establish the implemented state before changing
architecture.

## Global rules

- Do not introduce a platform dependency into portable code.
- Do not cross the Agent/Workload binary boundary with the Rust ABI.
- Do not make Core select physical A/B slots.
- Do not couple Agent OTA and Workload OTA into one transaction.
- Do not create an additional physical ESP flash owner.
- Native target-specific Workloads are the selected execution model. Wasm/wasmi are not.
- Treat the native Workload as trusted code unless memory isolation is explicitly added.
- Preserve the relevant `INV-*` invariants and run the relevant `BG-*` gates.

## Workflow

1. Start from the explicit base SHA.
2. Create a feature/fix/docs branch.
3. Identify affected invariants and contracts before coding.
4. Keep the change inside the requested scope.
5. Update the crate `README.md` when responsibilities, API, lifecycle, limitations,
   validation or dependency boundaries change.
6. Run `python3 tools/docs/docs_tool.py generate` after README changes.
7. Run focused tests and the referenced baseline gates.
8. Report branch, base, HEAD, changes, tests, hardware evidence, remaining gaps and debts.
9. STOP before merge and before beginning the next milestone unless explicitly instructed.

See `DOCUMENTATION.md` for the documentation contract.
