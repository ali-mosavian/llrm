#!/usr/bin/env python3
"""A target's link recipe, read from its `object.toml`: the one place that says how its objects are assembled and linked.

    linkrecipe.py TARGET assembler   the assembler's flag for the object format the target writes
    linkrecipe.py TARGET format      the linker's format words (`format dos`)
    linkrecipe.py TARGET FIELD       one `[link]` field (`first`, `last`, `final`, `options`, `loader`), words
"""

import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# jwasm's flag for each object format a target may write.
ASSEMBLER = {"omf": "-omf"}


def recipe(target: str) -> dict:
    """`target`'s `object.toml`, parsed."""
    with open(ROOT / "crates" / "target" / f"llrm-{target}" / "src" / "machines" / "object.toml", "rb") as text:
        return tomllib.load(text)


def assembler(target: str) -> str:
    """The flag that makes the assembler write `target`'s object format."""
    return ASSEMBLER[recipe(target)["writer"]]


def link(target: str, field: str) -> list[str]:
    """One field of `target`'s `[link]`, as the words a linker command takes."""
    value = recipe(target)["link"][field]
    return value if isinstance(value, list) else [value]


if __name__ == "__main__":
    target, field = sys.argv[1], sys.argv[2]
    print(" ".join([assembler(target)] if field == "assembler" else link(target, field)))
