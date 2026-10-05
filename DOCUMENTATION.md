# Documentation contract

IOBEWI uses a single-source documentation model.

For a crate, `README.md` is the canonical semantic documentation. The crate's
`AGENTS.md` is generated from that README and MUST NOT be edited manually. The human
documentation site is also generated from repository Markdown. `Cargo.toml` remains
canonical for package identity, features and dependency declarations; prose SHOULD NOT
duplicate machine-readable facts without a reason.

## Required crate README front matter

A crate README begins with this restricted YAML subset:

```yaml
---
layer: portable-service
status: implemented
invariants:
  - INV-001
gates:
  - BG-EXAMPLE
---
```

Required keys are `layer`, `status`, `invariants` and `gates`. Lists may be empty.
Package name and dependencies are derived from `Cargo.toml`.

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

Conditional sections such as Data flow, Lifecycle, Error model, Platform support and
Security and safety are encouraged when they add real information. Do not add empty
"Not applicable" sections.

## README -> AGENTS projection

The deterministic generator maps the canonical README into a compact operational view:

- Summary -> Role
- Responsibilities -> Owns
- Non-responsibilities -> Does not own
- Architecture -> Architecture position
- Public API -> Public contracts
- Invariants -> Invariants
- Data flow/Lifecycle -> Modification context
- Validation -> Required validation
- Known limitations -> Known limitations
- Related components -> Related components

The generator does not call an LLM and does not invent content.

## Generated outputs

Versioned:

- per-crate `AGENTS.md`, so a fresh clone is immediately agent-usable.

Generated on demand / CI and NOT versioned:

- `.generated/agent-index.json`
- `.generated/docs-site/`
- rendered static website output.

The agent index combines README front matter, crate path and Cargo package identity. It is
a navigation artifact, not a new source of truth.

## Global documents

- `ARCHITECTURE.md`: current cross-crate architecture.
- `INVARIANTS.md`: normative stable `INV-*` rules.
- root `AGENTS.md`: global agent workflow and read order.
- `docs/decisions/`: ADRs for accepted architectural decisions.
- `docs/contracts/`: index of canonical binary/persistent/network contracts.
- `docs/validation/`: reusable `BG-*` baseline gates.
- `docs/knowledge/`: current state, debt and roadmap.

## Change rules

Review/update the crate README whenever code changes its responsibility,
non-responsibility, public contract, lifecycle, validation requirement, limitation or
architecture position.

An architectural invariant change requires an ADR. A binary/network/persistent contract
change requires canonical contract and compatibility/validation review. A new crate is
incomplete until its README conforms and its generated AGENTS file is committed.

## Commands

```sh
python3 tools/docs/docs_tool.py audit
python3 tools/docs/docs_tool.py generate
python3 tools/docs/docs_tool.py check
python3 tools/docs/docs_tool.py site
```

`generate` refreshes versioned crate AGENTS files and the unversioned agent index.
`check` validates README structure/IDs and verifies that versioned AGENTS files exactly
match the deterministic projection. `site` builds MkDocs source under
`.generated/docs-site`.

The site projects crate `README.md` pages as `index.md` and rewrites relative
Markdown links to those pages, preserving fragments. Canonical repository links
continue to target `README.md`. Validate site changes with
`python3 -m mkdocs build --strict -f .generated/docs-site/mkdocs.yml` after `site`.
