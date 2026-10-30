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


def users_of(directory: Path, root: Path = ROOT) -> dict[str, list[str]]:
    """For each source file stem of `directory`, the files of the repository's crates that name it as a path (`stem::`) or import it (`use stem`): other crates
    through `llrm_x::stem::`. Tests and the crate roots, which only declare the module, do not count."""
    stems = {p.stem for p in sources(directory)}
    named = re.compile(r"\b(\w+)::")
    imported = re.compile(r"\buse (\w+)\b")
    found: dict[str, set[str]] = {stem: set() for stem in stems}
    for p in sorted((root / "crates").rglob("*.rs")) if (root / "crates").is_dir() else []:
        if p.stem.endswith("_tests") or p.stem in ("lib", "mod", "tests", "testing") or "/target/" in p.as_posix():
            continue
        text = p.read_text(encoding="utf-8", errors="replace")
        for stem in (set(named.findall(text)) | set(imported.findall(text))) & stems:
            if p.stem != stem:
                found[stem].add(p.stem)
    return {stem: sorted(users) for stem, users in found.items()}


def with_users(lines: list[str], users: list[str]) -> list[str]:
    """`lines`, and for a header that says neither where it runs nor what it provides, a `# Used by` section generated from the files that name it."""
    if not users or any(name in sections(lines) for name in ("Runs", "Used by", "Provides")):
        return lines
    return [*lines, "", "# Used by", "Named by " + ", ".join(f"`{one}`" for one in users) + "."]


def explained(lines: list[str]) -> bool:
    """Whether the header says how the file works: a `# How it works` or `# LLVM structure` section, or a prose paragraph after the summary
    (not a table, a code block or a list)."""
    if sections(lines) & {"How it works", "LLVM structure"}:
        return True
    paragraphs, para, fenced = [], [], False
    for line in lines:
        if line.lstrip().startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        if line.strip() and not line.startswith("#"):
            para.append(line.strip())
        elif para:
            paragraphs.append(para)
            para = []
    if para:
        paragraphs.append(para)
    return any(not p[0].startswith(("|", "-", "*")) and len(" ".join(p)) > 60 for p in paragraphs[1:])


def missing(lines: list[str], runs_or_provides: str, users: list[str] | None = None) -> list[str]:
    """What the header lacks: `summary`, `# How it works`, `# Runs`, `# Provides` or `# Example`. Where it runs may be derived from the files that name it."""
    gaps = [] if summary(lines) else ["summary"]
    have = sections(lines)
    if not explained(lines):
        gaps.append("# How it works")
    if "Example" not in have:
        gaps.append("# Example")
    if not users and not any(name in have for name in ("Runs", "Used by", "Provides")):
        gaps.append(f"# {runs_or_provides} (or # Used by)")
    return gaps


def check(required: list[str], root: Path) -> list[str]:
    """`file: gaps` for each file of the required directories (all three when none) with a gap."""
    found = []
    for _, _, directory, kind in PAGES:
        if required and directory not in required:
            continue
        users = users_of(root / directory, root)
        for path in sources(root / directory):
            gaps = missing(header(path), kind, users.get(path.stem))
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
