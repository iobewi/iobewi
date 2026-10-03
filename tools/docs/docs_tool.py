#!/usr/bin/env python3
"""IOBEWI single-source documentation validator/generator."""
from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GENERATED = ROOT / ".generated"
GENERATED_INDEX = GENERATED / "agent-index.json"
GEN_MARKER = "<!-- GENERATED FILE — DO NOT EDIT. Source: README.md -->"
REQUIRED_SECTIONS = [
    "Summary", "Responsibilities", "Non-responsibilities", "Architecture",
    "Public API", "Invariants", "Validation", "Known limitations",
    "Related components",
]
GLOBAL_MD = [
    "README.md", "ARCHITECTURE.md", "INVARIANTS.md", "DOCUMENTATION.md",
    "CONTRIBUTING.md",
]

def cargo_files():
    return sorted(p for p in ROOT.rglob("Cargo.toml") if ".generated" not in p.parts)

def load_toml(path: Path):
    with path.open("rb") as f:
        return tomllib.load(f)

def crates():
    out = []
    for cargo in cargo_files():
        doc = load_toml(cargo)
        package = doc.get("package")
        if package:
            out.append((cargo.parent, package))
    return out

def parse_front_matter(text: str):
    if not text.startswith("---\n"):
        raise ValueError("README must start with --- front matter")
    end = text.find("\n---\n", 4)
    if end < 0:
        raise ValueError("unterminated front matter")
    data, current = {}, None
    for line in text[4:end].splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        m = re.match(r"^([A-Za-z0-9_-]+):(?:\s*(.*))?$", line)
        if m:
            key, value = m.group(1), (m.group(2) or "").strip()
            current = key
            if value == "[]":
                data[key] = []
            elif value:
                data[key] = value.strip('"\'')
            else:
                data[key] = []
            continue
        m = re.match(r"^\s+-\s+(.+?)\s*$", line)
        if m and current:
            if not isinstance(data.get(current), list):
                raise ValueError(f"{current} is not a list")
            data[current].append(m.group(1).strip().strip('"\''))
            continue
        raise ValueError(f"unsupported front-matter line: {line!r}")
    return data, text[end + 5:]

def sections(body: str):
    result, title, current, buf = {}, None, None, []
    for line in body.splitlines():
        if line.startswith("# ") and title is None:
            title = line[2:].strip()
            continue
        if line.startswith("## "):
            if current is not None:
                result[current] = "\n".join(buf).strip()
            current, buf = line[3:].strip(), []
        elif current is not None:
            buf.append(line)
    if current is not None:
        result[current] = "\n".join(buf).strip()
    return title, result

def parse_readme(path: Path):
    meta, body = parse_front_matter(path.read_text(encoding="utf-8"))
    title, sec = sections(body)
    return meta, title, sec

def rel(path: Path):
    return path.relative_to(ROOT).as_posix()

def known_ids(path: Path, prefix: str):
    if not path.exists():
        return set()
    text = path.read_text(encoding="utf-8")
    if prefix == "BG":
        return set(re.findall(r"\bBG-[A-Z0-9]+(?:-[A-Z0-9]+)*\b", text))
    return set(re.findall(rf"\b{re.escape(prefix)}-\d{{3}}\b", text))

def agent_text(crate_dir: Path, package: dict, meta: dict, title: str, sec: dict):
    def part(name):
        return sec.get(name, "").strip()
    context = []
    for name in ("Data flow", "Lifecycle"):
        if part(name):
            context.append(f"### {name}\n\n{part(name)}")
    context_text = "\n\n".join(context) or "See the canonical README and implementation."
    return f"""# Agent Context — {title}

{GEN_MARKER}

- Package: `{package['name']}`
- Path: `{rel(crate_dir)}`
- Layer: `{meta['layer']}`
- Status: `{meta['status']}`

## Role

{part('Summary')}

## Owns

{part('Responsibilities')}

## Does not own

{part('Non-responsibilities')}

## Architecture position

{part('Architecture')}

## Public contracts

{part('Public API')}

## Invariants

{part('Invariants')}

## Modification context

{context_text}

## Required validation

{part('Validation')}

## Known limitations

{part('Known limitations')}

## Related components

{part('Related components')}

---

Canonical local documentation: `README.md`.
Package/features/dependencies: `Cargo.toml`.
Repository-wide rules: nearest parent/root `AGENTS.md`, `ARCHITECTURE.md`,
`INVARIANTS.md`, and referenced contracts/ADRs/gates.
"""

def build_agent_outputs():
    outputs = {}
    for crate_dir, package in crates():
        readme = crate_dir / "README.md"
        if not readme.exists():
            continue
        meta, title, sec = parse_readme(readme)
        outputs[crate_dir / "AGENTS.md"] = agent_text(crate_dir, package, meta, title, sec)
    return outputs

def build_index():
    index = []
    for crate_dir, package in crates():
        readme = crate_dir / "README.md"
        if not readme.exists():
            continue
        meta, title, _ = parse_readme(readme)
        index.append({
            "path": rel(crate_dir),
            "package": package["name"],
            "title": title,
            "layer": meta.get("layer"),
            "status": meta.get("status"),
            "invariants": meta.get("invariants", []),
            "gates": meta.get("gates", []),
            "readme": rel(readme),
            "agents": rel(crate_dir / "AGENTS.md"),
        })
    return json.dumps({"schema": 1, "crates": index}, indent=2) + "\n"

def validate():
    errors = []
    inv = known_ids(ROOT / "INVARIANTS.md", "INV")
    gates = known_ids(ROOT / "docs/validation/baseline-gates.md", "BG")
    found = crates()
    for crate_dir, package in found:
        readme = crate_dir / "README.md"
        if not readme.exists():
            errors.append(f"{rel(crate_dir)}: missing README.md")
            continue
        try:
            meta, title, sec = parse_readme(readme)
        except Exception as exc:
            errors.append(f"{rel(readme)}: {exc}")
            continue
        for key in ("layer", "status", "invariants", "gates"):
            if key not in meta:
                errors.append(f"{rel(readme)}: missing front-matter key {key}")
        if not title:
            errors.append(f"{rel(readme)}: missing H1 title")
        for name in REQUIRED_SECTIONS:
            if name not in sec:
                errors.append(f"{rel(readme)}: missing section ## {name}")
        for item in meta.get("invariants", []):
            if item not in inv:
                errors.append(f"{rel(readme)}: unknown invariant {item}")
        for item in meta.get("gates", []):
            if item not in gates:
                errors.append(f"{rel(readme)}: unknown gate {item}")
    return errors, found

def cmd_audit(_):
    errors, found = validate()
    print(f"crates={len(found)}")
    for crate_dir, package in found:
        state = "README" if (crate_dir / "README.md").exists() else "MISSING"
        print(f"{rel(crate_dir):45} {package['name']:34} {state}")
    print(f"validation_errors={len(errors)}")

def cmd_generate(_):
    errors, _ = validate()
    fatal = [e for e in errors if "missing README.md" in e or "front-matter" in e or "missing section" in e]
    if fatal:
        print("\n".join(fatal), file=sys.stderr)
        raise SystemExit(1)
    for path, content in build_agent_outputs().items():
        path.write_text(content, encoding="utf-8")
    GENERATED.mkdir(parents=True, exist_ok=True)
    GENERATED_INDEX.write_text(build_index(), encoding="utf-8")

def cmd_check(_):
    errors, found = validate()
    if not any("missing README.md" in e for e in errors):
        for path, content in build_agent_outputs().items():
            if not path.exists():
                errors.append(f"{rel(path)}: generated file missing")
            elif path.read_text(encoding="utf-8") != content:
                errors.append(f"{rel(path)}: generated file stale; run docs_tool.py generate")
    # Building the index is part of validation: failures here expose malformed metadata.
    if not errors:
        json.loads(build_index())
    if errors:
        for e in errors:
            print(e, file=sys.stderr)
        raise SystemExit(1)
    print(f"documentation check OK: {len(found)} crates")

def copy_page(src: Path, dst: Path):
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_text(src.read_text(encoding="utf-8"), encoding="utf-8")

def cmd_site(_):
    out = GENERATED / "docs-site"
    if out.exists():
        shutil.rmtree(out)
    content = out / "content"
    content.mkdir(parents=True)
    for name in GLOBAL_MD:
        src = ROOT / name
        if src.exists():
            copy_page(src, content / name)
    docs_root = ROOT / "docs"
    if docs_root.exists():
        for src in sorted(docs_root.rglob("*.md")):
            if "templates" in src.parts:
                continue
            copy_page(src, content / src.relative_to(ROOT))
    for crate_dir, package in crates():
        readme = crate_dir / "README.md"
        if readme.exists():
            copy_page(readme, content / crate_dir.relative_to(ROOT) / "index.md")
    GENERATED.mkdir(parents=True, exist_ok=True)
    GENERATED_INDEX.write_text(build_index(), encoding="utf-8")
    cfg = """site_name: IOBEWI
site_description: IOBEWI documentation generated from canonical repository Markdown
docs_dir: content
site_dir: site
use_directory_urls: true
theme:
  name: mkdocs
plugins:
  - search
"""
    (out / "mkdocs.yml").write_text(cfg, encoding="utf-8")
    print(out / "mkdocs.yml")

def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    for name, fn in (("audit", cmd_audit), ("generate", cmd_generate), ("check", cmd_check), ("site", cmd_site)):
        cmd = sub.add_parser(name)
        cmd.set_defaults(func=fn)
    args = parser.parse_args()
    args.func(args)

if __name__ == "__main__":
    main()
