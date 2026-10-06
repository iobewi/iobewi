# Contributing to IOBEWI

Read `ARCHITECTURE.md`, `INVARIANTS.md` and the nearest crate `AGENTS.md` before
changing code.

## Branches

Use normal development branches such as `feature/*`, `fix/*` and `docs/*`.
Historical `migration/*` branches are checkpoints, not the normal workflow.

## Documentation

Each crate's `README.md` is its canonical local documentation. Do not edit generated
crate `AGENTS.md` files. Change the README, run:

```sh
python3 tools/docs/docs_tool.py generate
python3 tools/docs/docs_tool.py check
```

and commit both the source README and generated outputs.

See `DOCUMENTATION.md` for the complete contract.

## Validation

Identify the `INV-*` invariants and `BG-*` gates referenced by the crate README.
Run focused tests first and the relevant baseline gates before requesting consolidation.

Do not merge a validated milestone or begin the next milestone without the explicit
workflow decision that authorizes it.

## Continuous integration triggers

Open a pull request (a draft is sufficient) to validate a feature branch. Rust and
documentation workflows run when a PR is opened, reopened or updated. Branch
pushes do not also launch another copy. Direct pushes and merges to main trigger
main validation; a local commit does not trigger Actions. Use workflow_dispatch
for an explicit branch run without a PR.

Each normal PR update runs five jobs: host, esp (locked), esp (latest),
esp-bootloader and knowledge. A newer update cancels obsolete runs of the same
PR/workflow; it does not cancel a different PR, main, manual or scheduled run.
The daily Rust schedule remains enabled to detect upstream dependency drift.
Distinct feature variants and the separate bootloader gate are retained; exact
duplicate host test commands and the empty esp-host job are removed.
