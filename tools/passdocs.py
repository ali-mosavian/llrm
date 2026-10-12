#!/usr/bin/env python3
"""Pass, analysis and backend reference, extracted from the `//!` header of each source file.

    tools/passdocs.py --out DIR                    write DIR/index.md, passes.md, analyses.md, backend.md
    tools/passdocs.py --check [--require DIR...]   list files lacking a summary or a required section

A header needs a summary line and `# How it works`, plus `# Runs` (transforms, backend) or `# Provides` (analyses).
`--check` exits 1 on a gap in a `--require` directory; without `--require` it only reports.
"""
from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = "https://github.com/ali-mosavian/llrm"
SKIP = {"lib", "mod", "testing"}

# page, title, source directory, the section that says where the file runs or what it gives
PAGES = [
    ("passes", "Passes", "crates/opt/llrm-transforms/src", "Runs"),
    ("analyses", "Analyses", "crates/opt/llrm-analysis/src", "Provides"),
    ("backend", "Backend phases", "crates/backend/llrm-core/src/backend", "Runs"),
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


def missing(lines: list[str], runs_or_provides: str) -> list[str]:
    """What the header lacks: `summary`, `# How it works`, `# Runs` or `# Provides`."""
    gaps = [] if summary(lines) else ["summary"]
    have = sections(lines)
    gaps += [f"# {name}" for name in ("How it works", runs_or_provides) if name not in have]
    return gaps


LINK = re.compile(r"\[(`[^`\]]+`)\](?:\[[^\]]*\]|\([\w:]+\))?(?!\()")


def render(lines: list[str]) -> list[str]:
    """Demote `# ` headings by two levels and turn rustdoc intra-doc links into plain code."""
    out, fenced = [], False
    for line in lines:
        if line.lstrip().startswith("```"):
            fenced = not fenced
        elif not fenced:
            if line.startswith("#"):
                line = "##" + line
            line = LINK.sub(r"\1", line)
        out.append(line)
    return out


def toc_line(stem: str, lines: list[str]) -> str:
    return f"- [{stem}](#{stem}): {summary(lines)}"


def section(rel: str, lines: list[str], sha: str) -> list[str]:
    """The page lines of one documented source file `rel`."""
    stem = Path(rel).stem
    return [f"## {stem}", "", f"Source: [`{rel}`]({REPO}/blob/{sha}/{rel})", "", *render(lines), ""]


def assemble(title: str, directory: str, toc: list[str], body: list[str], bare: list[str], sha: str) -> str:
    """Markdown of one page from its table of contents, sections and undocumented files."""
    out = [f"# {title}", "", f"From the `//!` headers of `{directory}`.", "", *toc, "", *body]
    if bare:
        out += ["## Undocumented", "", *[f"- [`{rel}`]({REPO}/blob/{sha}/{rel})" for rel in bare], ""]
    return "\n".join(out)


def index_line(name: str, title: str, documented: int, undocumented: int) -> str:
    return f"- [{title}]({name}.md): {documented} documented, {undocumented} undocumented"


def index_text(entries: list[str]) -> str:
    head = ["# Pass, analysis and backend reference", "", "Generated from the module header of each source file.", ""]
    return "\n".join([*head, *entries]) + "\n"


def page(title: str, directory: str, kind: str, root: Path, sha: str) -> tuple[str, int, int]:
    """Markdown of one page, and the counts of documented and undocumented files."""
    toc, body, bare = [], [], []
    for path in sources(root / directory):
        lines = header(path)
        rel = f"{directory}/{path.name}"
        if not lines:
            bare.append(rel)
            continue
        toc.append(toc_line(path.stem, lines))
        body += section(rel, lines, sha)
    return assemble(title, directory, toc, body, bare, sha), len(toc), len(bare)


def write(out: Path, root: Path, sha: str) -> None:
    out.mkdir(parents=True, exist_ok=True)
    entries = []
    for name, title, directory, kind in PAGES:
        text, done, bare = page(title, directory, kind, root, sha)
        (out / f"{name}.md").write_text(text, encoding="utf-8")
        entries.append(index_line(name, title, done, bare))
    (out / "index.md").write_text(index_text(entries), encoding="utf-8")


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
    ap.add_argument("--out", type=Path, help="write the pages here")
    ap.add_argument("--check", action="store_true", help="list files with a gap")
    ap.add_argument("--require", nargs="*", default=[], metavar="DIR", help="directories where a gap fails")
    args = ap.parse_args(argv)
    if not args.out and not args.check:
        ap.error("give --out or --check")
    if args.out:
        write(args.out, ROOT, os.environ.get("GITHUB_SHA") or "main")
    if args.check:
        found = check([d.rstrip("/") for d in args.require], ROOT)
        print("\n".join(found))
        return 1 if any(f.startswith(tuple(d.rstrip("/") + "/" for d in args.require)) for f in found) else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
