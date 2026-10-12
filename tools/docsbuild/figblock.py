"""Find the figures of a Markdown text: bare ```fig fences and the committed blocks around them.

A committed block is

    <!-- figure:SLUG -->
    ![ALT](path/to/SLUG.svg)

    <details><summary>Figure source</summary>

    ```fig
    ...
    ```

    </details>
    <!-- /figure -->

so GitHub shows the image with the source collapsed beneath it.
"""
from __future__ import annotations

import re
from dataclasses import dataclass

OPEN = re.compile(r"^<!-- figure:(\S+) -->\s*$")
CLOSE = re.compile(r"^<!-- /figure -->\s*$")
FENCE = re.compile(r"^(`{3,}|~{3,})\s*(\S*)")


@dataclass
class Fig:
    start: int  # first line of the region, 0-based
    end: int  # one past its last line
    spec: str
    spec_line: int  # document line (1-based) of the spec's first line


def _fence_end(lines: list[str], i: int) -> int:
    """Index of the line closing the fence opened at `i`, or len(lines)."""
    m = FENCE.match(lines[i])
    close = re.compile(rf"^{re.escape(m.group(1)[0])}{{{len(m.group(1))},}}\s*$")
    j = i + 1
    while j < len(lines) and not close.match(lines[j]):
        j += 1
    return j


def find(text: str) -> tuple[list[Fig], list[tuple[int, str]]]:
    """The figures of `text` and the problems (line, message) in their blocks."""
    lines = text.split("\n")
    figs: list[Fig] = []
    errs: list[tuple[int, str]] = []
    i = 0
    while i < len(lines):
        if OPEN.match(lines[i]):
            j = i + 1
            while j < len(lines) and not CLOSE.match(lines[j]):
                j += 1
            if j == len(lines):
                errs.append((i + 1, "figure block is never closed"))
                break
            k = next((x for x in range(i + 1, j) if (m := FENCE.match(lines[x])) and m.group(2) == "fig"), None)
            if k is None:
                errs.append((i + 1, "figure block has no ```fig fence"))
            else:
                e = _fence_end(lines, k)
                figs.append(Fig(i, j + 1, "\n".join(lines[k + 1 : e]), k + 2))
            i = j + 1
            continue
        m = FENCE.match(lines[i])
        if m:
            e = _fence_end(lines, i)
            if m.group(2) == "fig" and e < len(lines):
                figs.append(Fig(i, e + 1, "\n".join(lines[i + 1 : e]), i + 2))
            i = e + 1
            continue
        i += 1
    return figs, errs


def unwrap(text: str) -> str:
    """`text` with each committed block replaced by its bare fence, line count unchanged."""
    figs, _ = find(text)
    lines = text.split("\n")
    for f in reversed(figs):
        bare = ["```fig", *f.spec.split("\n"), "```"]
        lead = f.spec_line - 2 - f.start  # keeps the spec's first line where it was
        lines[f.start : f.end] = [""] * lead + bare + [""] * (f.end - f.start - lead - len(bare))
    return "\n".join(lines)
