#!/usr/bin/env python3
"""Rewrite figure references and ```fig fences as inline ```mermaid fences.

    mdfigs.py --convert           every tracked Markdown file: an image of a figure whose NAME.fig sits beside
                                  the image becomes a mermaid fence plus an italic caption; NAME.fig and
                                  NAME.png are deleted; a ```fig fence is converted in place
    mdfigs.py --headers FILE...   the same for ```fig fences in the `//!` headers of Rust files

Running it again changes nothing.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent))

from docsbuild import tomermaid  # noqa: E402

MD_IMAGE = re.compile(r"!\[(?P<alt>[^\]]*)\]\((?P<src>[^)\s]+\.png)\)")
HTML_IMAGE = re.compile(r"<img\b[^>]*?\bsrc=\"(?P<src>[^\"]+\.png)\"[^>]*>")
ALT = re.compile(r"\balt=\"([^\"]*)\"")
FENCE = re.compile(r"^(?P<ind>\s*)(?P<q>`{3,}|~{3,})\s*(?P<lang>\S*)\s*$")


def caption_line(res: tomermaid.Result, alt: str = "") -> str:
    cap = res.caption()
    if not res.title and alt:
        cap = " ".join(p for p in (f"*{alt}*", res.extra) if p)
    return cap


def block(res: tomermaid.Result, alt: str = "") -> list[str]:
    out = ["```mermaid", *res.text.split("\n"), "```"]
    cap = caption_line(res, alt)
    if cap:
        out += ["", cap]
    return out


def convert_markdown(text: str, here: Path) -> tuple[str, list[Path], list[str]]:
    """`text` of the document in directory `here` with figure images and fences as mermaid.

    Returns the new text, the .fig and .png files that are now unused, and the problems found.
    """
    lines = text.split("\n")
    gone: list[Path] = []
    errors: list[str] = []
    i, fenced = 0, None
    while i < len(lines):
        line = lines[i]
        m = FENCE.match(line)
        if fenced:
            if m and m.group("q")[0] == fenced[0] and len(m.group("q")) >= len(fenced) and not m.group("lang"):
                fenced = None
            i += 1
            continue
        if m:
            if m.group("lang") == "fig" and not m.group("ind"):
                j = i + 1
                while j < len(lines) and not re.match(rf"^{re.escape(m.group('q'))}\s*$", lines[j]):
                    j += 1
                try:
                    res = tomermaid.convert("\n".join(lines[i + 1 : j]))
                except tomermaid.FigureError as e:
                    errors.append(f"line {i + 2 + e.line - 1}: {e.message}")
                    i = j + 1
                    continue
                new = block(res)
                lines[i : j + 1] = new
                lines = _padded(lines, i, len(new))
                continue
            fenced = m.group("q")
            i += 1
            continue
        hit = None
        stripped = re.sub(r"`[^`]*`", lambda k: " " * len(k.group(0)), line)
        for pat in (MD_IMAGE, HTML_IMAGE):
            for mm in pat.finditer(stripped):
                fig = here / mm.group("src")
                spec = fig.with_suffix(".fig")
                if spec.is_file():
                    hit = (mm, spec, fig)
                    break
            if hit:
                break
        if not hit:
            i += 1
            continue
        mm, spec, png = hit
        alt = mm.groupdict().get("alt")
        if alt is None:
            a = ALT.search(mm.group(0))
            alt = a.group(1) if a else ""
        try:
            res = tomermaid.convert(spec.read_text(encoding="utf-8"))
        except tomermaid.FigureError as e:
            errors.append(f"{spec}: line {e.line}: {e.message}")
            i += 1
            continue
        new = block(res, alt)
        before, after = line[: mm.start()].rstrip(), line[mm.end() :].strip()
        repl = ([before, ""] if before else []) + new + (["", after] if after else [])
        lines[i : i + 1] = repl
        lines = _padded(lines, i + (2 if before else 0), len(new))
        gone += [p for p in (spec, png) if p not in gone]
        i += len(repl)
    return "\n".join(lines), gone, errors


def _padded(lines: list[str], start: int, n: int) -> list[str]:
    """`lines` with a blank line before and after the n lines at `start`, when missing."""
    if start + n < len(lines) and lines[start + n].strip():
        lines.insert(start + n, "")
    if start > 0 and lines[start - 1].strip():
        lines.insert(start, "")
    return lines


def convert_header(text: str) -> tuple[str, list[str]]:
    """Rust source `text` with the ```fig fences of its leading `//!` block as mermaid fences."""
    lines = text.split("\n")
    end = 0
    while end < len(lines) and (lines[end].lstrip().startswith("//!") or (not lines[end].strip() and end == 0)):
        end += 1
    inner = []
    for raw in lines[:end]:
        s = raw.lstrip()
        if not s.startswith("//!"):
            inner.append("")
            continue
        s = s[3:]
        inner.append(s[1:] if s.startswith(" ") else s)
    new, _gone, errors = convert_markdown("\n".join(inner), Path("/nonexistent"))
    if new == "\n".join(inner):
        return text, errors
    out = [("//! " + x) if x else "//!" for x in new.split("\n")]
    return "\n".join(out + lines[end:]), errors


def tracked(root: Path, pattern: str) -> list[Path]:
    out = subprocess.run(["git", "ls-files", pattern], cwd=root, capture_output=True, text=True, check=True).stdout
    return [root / p for p in out.splitlines() if (root / p).is_file()]


def convert_tree(root: Path, files: list[Path] | None = None) -> tuple[int, list[str]]:
    """Convert every tracked Markdown file; returns (documents changed, problems)."""
    changed, problems = 0, []
    for path in files if files is not None else tracked(root, "*.md"):
        old = path.read_text(encoding="utf-8")
        new, gone, errors = convert_markdown(old, path.parent)
        problems += [f"{path.relative_to(root)}: {e}" for e in errors]
        if new != old:
            path.write_text(new, encoding="utf-8")
            changed += 1
        for g in gone:
            g.unlink(missing_ok=True)
    return changed, problems


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--convert", action="store_true")
    ap.add_argument("--headers", nargs="+", type=Path, metavar="FILE")
    args = ap.parse_args(argv)
    if not (args.convert or args.headers):
        ap.error("give --convert or --headers")
    problems: list[str] = []
    if args.convert:
        n, problems = convert_tree(ROOT)
        print(f"{n} document(s) rewritten")
    for f in args.headers or []:
        old = f.read_text(encoding="utf-8")
        new, errors = convert_header(old)
        problems += [f"{f}: {e}" for e in errors]
        if new != old:
            f.write_text(new, encoding="utf-8")
            print(f"{f}: header rewritten")
    print("\n".join(problems))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
