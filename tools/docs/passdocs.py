#!/usr/bin/env python3
"""Pass, analysis and backend headers: which source files have one, and what each lacks.

    tools/docs/passdocs.py --check [--require DIR...]   list files lacking a summary or a required section

A header needs a summary line and `# How it works`, plus one of `# Runs` (a pass), `# Used by` (a helper) or `# Provides` (an analysis), and `# Example`.
`--check` exits 1 on a gap in a `--require` directory; without `--require` it only reports.
The pages are written from these headers by `tools/docs/docsbuild/build.py --sync`.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPO = "https://github.com/ali-mosavian/llrm"
SKIP = {"lib", "mod", "testing"}

# page, title, source directory, the section that says where the file runs or what it gives
PAGES = [
    ("passes", "Passes", "crates/opt/llrm-transforms/src", "Runs"),
    ("analyses", "Analyses", "crates/opt/llrm-analysis/src", "Provides"),
    ("backend", "Backend phases", "crates/codegen/llrm-codegen/src/backend", "Runs"),
]


def sources(directory: Path) -> list[Path]:
    """Non-test `.rs` files of `directory`, sorted, without lib/mod/testing."""
    return sorted(
        p for p in directory.glob("*.rs") if not p.stem.endswith("_tests") and p.stem not in SKIP
    )


def header_of(text: str) -> list[str]:
    """The leading `//!` block of source `text`, prefix stripped."""
    lines: list[str] = []
    for raw in text.splitlines():
        stripped = raw.strip()
        if not stripped.startswith("//!"):
            if lines or stripped:
                break
            continue
        stripped = stripped[3:]
        lines.append(stripped[1:] if stripped.startswith(" ") else stripped)
    return lines


def header(path: Path) -> list[str]:
    """The leading `//!` block of `path`, prefix stripped."""
    return header_of(path.read_text(encoding="utf-8"))


def summary(lines: list[str]) -> str:
    """The first sentence of the first paragraph, unwrapped."""
    para: list[str] = []
    for line in lines:
        if not line.strip():
            if para:
                break
            continue
        para.append(line.strip())
    text = " ".join(para)
    return re.split(r"(?<=\.)\s", text, maxsplit=1)[0]


def sections(lines: list[str]) -> set[str]:
    """Names of the `# ` headings outside code fences."""
    found, fenced = set(), False
    for line in lines:
        if line.lstrip().startswith("```"):
            fenced = not fenced
        elif not fenced and line.startswith("# "):
            found.add(line[2:].strip())
    return found


def section_line(lines: list[str], names: tuple[str, ...]) -> str:
    """The first non-blank line under the first of the `# ` headings `names` that the header has, else ''."""
    for name in names:
        fenced, inside = False, False
        for line in lines:
            if line.lstrip().startswith("```"):
                fenced = not fenced
            if not fenced and line.startswith("# "):
                inside = line[2:].strip() == name
            elif inside and not fenced and line.strip() and not line.lstrip().startswith("```"):
                return line.strip()
    return ""


def missing(lines: list[str], runs_or_provides: str) -> list[str]:
    """What the header lacks: `summary`, `# How it works`, `# Runs`, `# Provides` or `# Example`."""
    gaps = [] if summary(lines) else ["summary"]
    have = sections(lines)
    gaps += [f"# {name}" for name in ("How it works", "Example") if name not in have]
    if not any(name in have for name in ("Runs", "Used by", "Provides")):
        gaps.append(f"# {runs_or_provides} (or # Used by)")
    return gaps


def check(required: list[str], root: Path) -> list[str]:
    """`file: gaps` for each file of the required directories (all three when none) with a gap."""
    found = []
    for _, _, directory, kind in PAGES:
        if required and directory not in required:
            continue
        for path in sources(root / directory):
            gaps = missing(header(path), kind)
            if gaps:
                found.append(f"{directory}/{path.name}: missing {', '.join(gaps)}")
    return found


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="list files with a gap")
    ap.add_argument("--require", nargs="*", default=[], metavar="DIR", help="directories where a gap fails")
    args = ap.parse_args(argv)
    if not args.check:
        ap.error("give --check")
    found = check([d.rstrip("/") for d in args.require], ROOT)
    print("\n".join(found))
    return 1 if any(f.startswith(tuple(d.rstrip("/") + "/" for d in args.require)) for f in found) else 0


if __name__ == "__main__":
    sys.exit(main())
