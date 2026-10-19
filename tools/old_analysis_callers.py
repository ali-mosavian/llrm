#!/usr/bin/env python3
"""The old-analysis callers under parity: who still asks the stand-in analyses that a translation replaces.

    tools/old_analysis_callers.py [--root DIR] [--list FILE]

Static: reads the source under crates/, no build. Lists, per old analysis (tools/old_analyses.toml), the callers as
file:line grouped by pass (the file), then ends with the one number:
`old-analysis callers under parity: N (analyses with old callers: M)`, N call sites, M analyses with at least one.
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


def pattern(entry: dict) -> re.Pattern[str]:
    """`ask`: a manager analysis asked as `get::<T>`; `use`: a name used in code; `module`: a call `module::f`."""
    if "ask" in entry:
        return re.compile(rf"\b\w+::<(?:\w+::)*{entry['ask']}>")
    if "module" in entry:
        return re.compile(rf"\b{entry['module']}::\w")
    return re.compile(rf"\b{entry['use']}\b")


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


def report(found: dict[str, dict[str, list[int]]]) -> list[str]:
    lines: list[str] = []
    for name, passes in found.items():
        total = sum(len(rows) for rows in passes.values())
        lines.append(f"{name}: {total} callers in {len(passes)} passes")
        for relative, rows in passes.items():
            lines.append(f"  {relative}: {', '.join(str(row) for row in rows)}")
    sites = sum(len(rows) for passes in found.values() for rows in passes.values())
    open_ = sum(1 for passes in found.values() if passes)
    lines.append(f"old-analysis callers under parity: {sites} (analyses with old callers: {open_})")
    return lines


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--list", type=Path, default=LIST)
    args = parser.parse_args(argv)
    entries = tomllib.loads(args.list.read_text())["analysis"]
    print("\n".join(report(callers(args.root, entries))))
    return 0


if __name__ == "__main__":
    sys.exit(main())
