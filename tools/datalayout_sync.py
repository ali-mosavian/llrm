#!/usr/bin/env python3
"""The datalayout strings every target's `datalayout.toml` states, and the copies of them elsewhere.

    datalayout_sync.py --check   exit 1 and name each copy that differs from its description
    datalayout_sync.py --sync    rewrite every such copy from the description

`crates/target/llrm-x86-m*/src/machines/datalayout.toml` is the one place that says a target's datalayout. Tests, fixtures, the cc
driver's table and tools spell it too (the f80:16 change took a sed over 64 sites). A copy is a token that opens as a target's
description does, to its sixth field; it is checked against the description, and an `-ni:` suffix, which some copies add, is kept.
Abbreviated spellings (no `p3`/`p4`) are other strings and are left alone.
"""
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# LLVM's own test inputs keep the pin's spelling of x86-64's datalayout (no `i128:128`), whatever llrm's description says.
PORTED = (
    "crates/opt/llrm-transforms/tests/",
    # Generated IR (emitted by the frontends, optimized by the passes): it says the layout the compiler made it with, so a copy there is output, not a source to keep in step.
    "crates/opt/llrm-analysis/corpus/",
)
# Files whose old spelling is the point: a test that an old layout means what the new one states, and this tool's own test of rewriting an old copy.
# LLVM IR and MIR files must carry a datalayout line: they are data, not copies to fold into the description.
FIXTURE_SUFFIXES = (".ll", ".mir")
KEPT = ("crates/ir/llrm-mir/src/datalayout.rs", "tools/test_datalayout_sync.py")


def descriptions(root: Path = ROOT) -> list[str]:
    """Each target's datalayout string, from its `datalayout.toml`."""
    found = []
    for path in sorted((root / "crates" / "target").glob("llrm-*/src/machines/datalayout.toml")):
        with open(path, "rb") as text:
            found.append(tomllib.load(text)["datalayout"])
    return found


def anchor(layout: str) -> str:
    """What makes a token a copy of `layout`: its first six fields (arm64 and x86-64 share four), so an older spelling is still found."""
    return "-".join(layout.split("-")[:6]) + "-"


def rewritten(text: str, layouts: list[str]) -> str:
    """`text` with each copy of a description replaced by the description (an `-ni:` suffix kept)."""
    for layout in layouts:
        # A description with fewer fields than its anchor names has no safe anchor.
        if layout.count("-") < 6:
            continue

        def swap(match: re.Match, layout: str = layout) -> str:
            suffix = re.search(r"-ni:[0-9:]+$", match.group(0))
            return layout + (suffix.group(0) if suffix else "")

        text = re.sub(re.escape(anchor(layout)) + r"[A-Za-z0-9:_-]*", swap, text)
    return text


def tracked(root: Path = ROOT) -> list[Path]:
    out = subprocess.check_output(["git", "ls-files", "-z"], cwd=root, text=True)
    return [root / name for name in out.split("\0") if name]


def stale(root: Path = ROOT) -> list[str]:
    """The tracked text files that hold a copy differing from its description."""
    layouts = descriptions(root)
    found = []
    for path in tracked(root):
        if path.is_symlink() or not path.is_file() or str(path.relative_to(root)).startswith(PORTED + KEPT):
            continue
        try:
            text = path.read_text()
        except (UnicodeDecodeError, OSError):
            continue
        if rewritten(text, layouts) != text:
            found.append(str(path.relative_to(root)))
    return found


def copies(root: Path = ROOT) -> dict[str, int]:
    """Per tracked text file, how many copies of a description's string it holds (the description files are the source, not copies; `.ll`/`.mir` fixtures are data and not counted)."""
    layouts = [one for one in descriptions(root) if one.count("-") >= 6]
    pattern = re.compile("|".join(re.escape(anchor(one)) for one in layouts))
    found = {}
    for path in tracked(root):
        name = str(path.relative_to(root))
        if path.is_symlink() or not path.is_file() or name.endswith("/datalayout.toml") or name.endswith(FIXTURE_SUFFIXES) or name.startswith(PORTED + KEPT) or name == "tools/test_datalayout_copies.py":
            continue
        try:
            n = len(pattern.findall(path.read_text()))
        except (UnicodeDecodeError, OSError):
            continue
        if n:
            found[name] = n
    return found


def main(argv: list[str]) -> int:
    if argv[1:] == ["--sync"]:
        layouts = descriptions()
        for name in stale():
            path = ROOT / name
            path.write_text(rewritten(path.read_text(), layouts))
            print(f"synced {name}")
        return 0
    differing = stale()
    for name in differing:
        print(f"{name}: a datalayout copy differs from its datalayout.toml; run tools/datalayout_sync.py --sync")
    return 1 if differing else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
