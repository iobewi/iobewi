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
