"""docs/architecture/readme.md's Contents list, generated from the pages' front matter.

A page joins the list with a block at the top of its file:

    ---
    architecture-index: GVN as a translation of LLVM's
    architecture-order: 480
    ---

Entries run by order, then path. The list sits between the two marker comments of the readme, so a docs PR adds its own page and never
edits the shared line (two PRs adding one each conflicted on it every time).
"""
from __future__ import annotations

import os
from pathlib import Path

START, END = "<!-- architecture-index:start -->", "<!-- architecture-index:end -->"
README = Path("docs/architecture/readme.md")
EXTERNAL = [("packages design", "https://github.com/ali-mosavian/llrm/issues/1526", 9000)]  # a link with no page behind it


def front_matter(text: str) -> dict[str, str]:
    """The `key: value` lines of a block that opens the file with `---`; empty where there is none."""
    lines = text.split("\n")
    if not lines or lines[0] != "---" or "---" not in lines[1:]:
        return {}
    out = {}
    for line in lines[1 : lines.index("---", 1)]:
        key, _, value = line.partition(":")
        if _:
            out[key.strip()] = value.strip()
    return out


def entries(root: Path) -> list[tuple[int, str, str, str]]:
    """(order, path, text, target) of every page under docs/ that names itself in the index, target relative to the readme."""
    found = []
    for path in sorted((root / "docs").rglob("*.md")):
        meta = front_matter(path.read_text())
        if "architecture-index" in meta:
            rel = path.relative_to(root)
            found.append((int(meta.get("architecture-order", 5000)), str(rel), meta["architecture-index"], os.path.relpath(path, (root / README).parent)))
    found += [(order, target, text, target) for text, target, order in EXTERNAL]
    return sorted(found)


def render(root: Path) -> str:
    return "Contents: " + ", ".join(f"[{text}]({target})" for _, _, text, target in entries(root))


def synced(readme: str, root: Path) -> str:
    """`readme` with the text between the markers replaced by the current list."""
    head, _, rest = readme.partition(START)
    _, _, tail = rest.partition(END)
    if not _ and START not in readme:
        raise SystemExit(f"{README} has no {START} ... {END} markers")
    return f"{head}{START}\n{render(root)}\n{END}{tail}"


def drift(root: Path) -> bool:
    if not (root / README).exists():
        return False  # a tree with no architecture readme (a test's) has no list to be stale
    text = (root / README).read_text()
    return synced(text, root) != text


def sync(root: Path) -> bool:
    """Write the readme; True where it changed."""
    if not (root / README).exists():
        return False
    text = (root / README).read_text()
    new = synced(text, root)
    if new != text:
        (root / README).write_text(new)
    return new != text
