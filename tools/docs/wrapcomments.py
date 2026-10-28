"""Wrap the comments of C files at 80 columns (clang-format has one column
limit, 100 for code).  A comment is `/* ... */` or a run of `//` lines; its
paragraphs (split at blank lines) are filled again, keeping the first line's
indent and the continuation indent.  A comment after code that runs past 80
moves above the line.

    python tools/docs/wrapcomments.py FILE...
"""

from __future__ import annotations

import re
import sys
import textwrap
from pathlib import Path

LIMIT = 80


def fill(text: str, first: str, rest: str) -> list[str]:
    lines: list[str] = []
    for at, paragraph in enumerate(re.split(r"\n\s*\n", text.strip())):
        if at:
            lines.append("")
        words = " ".join(paragraph.split())
        lines += textwrap.wrap(
            words,
            LIMIT,
            initial_indent=first if not lines else rest,
            subsequent_indent=rest,
            break_long_words=False,
            break_on_hyphens=False,
        )
    return lines


def wrap_block(indent: str, body: str, closed_inline: bool) -> list[str]:
    text = re.sub(r"\n\s*", "\n", body.strip("\n"))
    text = re.sub(r"(?<=\S)\n(?=\S)", " ", text)
    first = f"{indent}/* "
    rest = f"{indent}   "
    lines = fill(text, first, rest)
    lines[-1] += " */"
    if len(lines[-1]) > LIMIT:
        lines[-1] = lines[-1][:-3]
        lines.append(f"{rest}*/")
    return lines


def process(source: str) -> str:
    out: list[str] = []
    lines = source.split("\n")
    at = 0
    while at < len(lines):
        line = lines[at]
        stripped = line.lstrip()
        indent = line[: len(line) - len(stripped)]
        if stripped.startswith("/*"):
            end = at
            while "*/" not in lines[end]:
                end += 1
            block = lines[at : end + 1]
            if all(len(one) <= LIMIT for one in block):
                out += block
            else:
                joined = "\n".join(block)
                body = joined.strip()[2:-2]
                # a `*` margin, if the comment has one
                body = re.sub(r"\n\s*\*(?!/)", "\n", body)
                out += wrap_block(indent, body, False)
            at = end + 1
            continue
        match = re.match(r"^(.*\S)\s+(/\*.*\*/|//.*)$", line)
        if len(line) > LIMIT and match and not stripped.startswith("//") and match.group(1).count('"') % 2 == 0:
            code, comment = match.groups()
            body = comment[2:-2] if comment.startswith("/*") else comment[2:]
            out += wrap_block(indent, body, True)
            out.append(code)
            at += 1
            continue
        if stripped.startswith("//") and len(line) > LIMIT:
            run = [line]
            while at + 1 < len(lines) and lines[at + 1].lstrip().startswith("//"):
                at += 1
                run.append(lines[at])
            body = "\n".join(one.lstrip()[2:] for one in run)
            text = "\n".join(re.sub(r"\n\s*", " ", p) for p in re.split(r"\n\s*\n", body.strip()))
            out += [one for one in fill(text, f"{indent}// ", f"{indent}// ")]
            at += 1
            continue
        out.append(line)
        at += 1
    return "\n".join(out)


def main() -> int:
    for name in sys.argv[1:]:
        path = Path(name)
        text = path.read_text()
        new = process(text)
        if new != text:
            path.write_text(new)
    return 0


if __name__ == "__main__":
    sys.exit(main())
