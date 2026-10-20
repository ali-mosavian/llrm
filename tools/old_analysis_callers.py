#!/usr/bin/env python3
"""The old-analysis callers under parity: who still asks the stand-in analyses that a translation replaces.

    tools/old_analysis_callers.py [--root DIR] [--list FILE]

Static: reads the source under crates/, no build. Lists, per old analysis (the Old rows of parity.rs's ANALYSES, plus the modules and
types in tools/old_analyses.toml), the callers as
file:line grouped by pass (the file), then ends with the one number:
`old-analysis callers under parity: N (X 222; without it N-222); analyses with old callers: M (M-1 without X)`: N call sites, M analyses
with at least one, X the entries flagged `moves_last` (MemRef: the shared location type that moves last), split out.
A caller is a code line in the entry's `scope` (default crates/) outside the analysis' own files, tests (`*_tests.rs`, `tests/`, a `#[cfg(test)]` module at
column 0), comments and `use` lines. A file an entry lists in `defined_in` is not a caller of it.
"""
from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIST = Path(__file__).resolve().parent / "old_analyses.toml"
REGISTRY = "crates/opt/llrm-transforms/src/parity.rs"
OLD_ROW = re.compile(r'old_analysis\(\s*"([^"]+)",\s*"(\w+)"')


def pattern(entry: dict) -> re.Pattern[str]:
    """`ask`: a manager analysis asked as `get::<T>`; `use`: a name used in code; `module`: a call `module::f`."""
    if "ask" in entry:
        return re.compile(rf"\b\w+::<(?:\w+::)*{entry['ask']}>|<(?:\w+::)*{entry['ask']} as ")
    if "module" in entry:
        return re.compile(rf"\b{entry['module']}::\w")
    return re.compile(rf"\b{entry['use']}\b")


def registry_entries(root: Path) -> list[dict]:
    """The Old rows of parity.rs's ANALYSES as `ask` entries named by their type. A row whose type no source defines (the four
    counters) is skipped. An ask is `get::<T>`; a definition never matches, so the defining file still counts its own asks."""
    text = (root / REGISTRY).read_text()
    sources_text = {relative: path.read_text(errors="replace") for relative, path in sources(root)}
    entries = []
    for _key, symbol in OLD_ROW.findall(text):
        defining = re.compile(rf"\b(?:struct|enum|type)\s+{symbol}\b|\bimpl\b[^\n{{]*\bfor\s+{symbol}\b")
        if any(defining.search(body) for body in sources_text.values()):
            entries.append({"name": symbol, "ask": symbol})
    return entries


def code_lines(text: str):
    """(line number, line) of the non-test code: no comments, no `use`, nothing after a `#[cfg(test)]` at column 0."""
    for number, line in enumerate(text.splitlines(), 1):
        if line.startswith("#[cfg(test)]"):
            return
        stripped = line.lstrip()
        if stripped.startswith("//") or stripped.startswith("use ") or stripped.startswith("pub use "):
            continue
        yield number, line


def sources(root: Path):
    for path in sorted((root / "crates").rglob("*.rs")):
        relative = path.relative_to(root).as_posix()
        if path.name.endswith("_tests.rs") or path.name == "tests.rs" or "/tests/" in relative or "/benches/" in relative:
            continue
        yield relative, path


def callers(root: Path, entries: list[dict]) -> dict[str, dict[str, list[int]]]:
    """analysis -> pass file -> line numbers."""
    found: dict[str, dict[str, list[int]]] = {entry["name"]: {} for entry in entries}
    patterns = {entry["name"]: pattern(entry) for entry in entries}
    skipped = {entry["name"]: set(entry.get("defined_in", [])) for entry in entries}
    scope = {entry["name"]: tuple(entry.get("scope", ["crates/"])) for entry in entries}
    for relative, path in sources(root):
        text = path.read_text(errors="replace")
        for number, line in code_lines(text):
            for name, regex in patterns.items():
                if relative.startswith(scope[name]) and relative not in skipped[name] and regex.search(line):
                    found[name].setdefault(relative, []).append(number)
    return found


def report(found: dict[str, dict[str, list[int]]], last: frozenset[str] = frozenset()) -> list[str]:
    """The per-analysis table, then the headline; `last` names the analyses that move last (MemRef), split out."""
    lines: list[str] = []
    for name, passes in found.items():
        total = sum(len(rows) for rows in passes.values())
        lines.append(f"{name}: {total} callers in {len(passes)} passes")
        for relative, rows in passes.items():
            lines.append(f"  {relative}: {', '.join(str(row) for row in rows)}")
    sites = {name: sum(len(rows) for rows in passes.values()) for name, passes in found.items()}
    total, open_ = sum(sites.values()), sum(1 for count in sites.values() if count)
    head = f"old-analysis callers under parity: {total}"
    tail = f"analyses with old callers: {open_}"
    if last & found.keys():
        held = sum(sites[name] for name in last & found.keys())
        open_held = sum(1 for name in last & found.keys() if sites[name])
        names = "/".join(sorted(last & found.keys()))
        head += f" ({names} {held}; without it {total - held})"
        tail += f" ({open_ - open_held} without {names})"
    lines.append(f"{head}; {tail}")
    return lines


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--list", type=Path, default=LIST)
    args = parser.parse_args(argv)
    entries = registry_entries(args.root) + tomllib.loads(args.list.read_text())["analysis"]
    last = frozenset(entry["name"] for entry in entries if entry.get("moves_last"))
    print("\n".join(report(callers(args.root, entries), last)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
