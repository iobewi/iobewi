# Documentation contract

IOBEWI uses a single-source documentation model.

For a crate, `README.md` is the canonical semantic documentation. The crate's
`AGENTS.md` is generated from that README and MUST NOT be edited manually. The human
documentation site is also generated from repository Markdown. `Cargo.toml` remains
canonical for package identity, features and dependency declarations; prose MUST NOT
duplicate machine-readable facts without a reason.

## Required crate README front matter

A crate README begins with this restricted YAML subset:

```yaml
---
layer: portable-service
status: stable
invariants:
  - INV-001
gates:
  - BG-EXAMPLE
---
```

Required keys:

- `layer`
- `status`
- `invariants` (may be empty)
- `gates` (may be empty)

Package name and dependencies are derived from `Cargo.toml` and are deliberately not
duplicated in the front matter.

## Required sections

Every crate README MUST contain:

- `# <title>`
- `## Summary`
- `## Responsibilities`
- `## Non-responsibilities`
- `## Architecture`
- `## Public API`
- `## Invariants`
- `## Validation`
- `## Known limitations`
- `## Related components`

Conditional sections are encouraged when relevant:

- `## Data flow`
- `## Lifecycle`
- `## Error model`
- `## Platform support`
- `## Security and safety`

Do not add empty "Not applicable" sections merely to satisfy a template.

## README -> AGENTS projection

The generator deterministically maps:

- Summary -> Role
- Responsibilities -> Owns
- Non-responsibilities -> Does not own
- Architecture -> Architecture position
- Public API -> Public contracts
- Invariants -> Invariants
- Lifecycle/Data flow -> Modification context when present
- Validation -> Required validation
- Known limitations -> Known limitations
- Related components -> Related components

It does not call an LLM and does not invent content.

## Global documents

Repository-wide rules live at the root or under `docs/`:

- `ARCHITECTURE.md`: current cross-crate architecture.
- `INVARIANTS.md`: normative stable `INV-*` rules.
- `AGENTS.md`: global agent workflow and read order.
- `docs/decisions/`: ADRs explaining accepted architectural choices.
- `docs/contracts/`: index of canonical binary/persistent/network contracts.
- `docs/validation/`: reusable `BG-*` baseline gates.
- `docs/knowledge/`: current state, debts and roadmap.

Crate README files reference global IDs instead of copying their complete text.

## Change rules

Review/update the crate README whenever code changes its responsibility, non-responsibility,
public contract, lifecycle, validation requirement, limitation or architecture position.

An architectural invariant change requires an ADR. A binary/network/persistent contract
change requires its canonical contract documentation and compatibility/validation review.
A new crate is incomplete until its README conforms and its generated AGENTS file is
committed.

## Commands

```sh
python3 tools/docs/docs_tool.py audit
python3 tools/docs/docs_tool.py generate
python3 tools/docs/docs_tool.py check
python3 tools/docs/docs_tool.py site
```

`generate` writes crate `AGENTS.md` files and `docs/generated/agent-index.json`.
`check` verifies that generated outputs are current. `site` creates an MkDocs source
tree under `.generated/docs-site`; generated site output is not a source of truth.
