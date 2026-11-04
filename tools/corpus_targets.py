#!/usr/bin/env python3
"""Which targets a tests/run/c program is for: its `// targets:` line (in the first lines, after `// flags:` and `// link:`).

A program with no line is for every target. Tokens: `dos` (x86_16 and x86_32 DOS/Watcom targets), a family (`x86-m16`,
`x86-m32`, `x86-m64`) or a full target name. A harness asks `applies`, and prints `NA` for a program that does not, never
FAIL: `tools/corpus_targets.py check TARGET FILE.c` exits 3 and prints "n/a, DOS-only" (or "n/a, targets: ...").
"""
import re
import sys
from pathlib import Path

HEADER_LINES = 6
DOS = re.compile(r"^x86_(16|32)-(dos|wat)")
FAMILY = re.compile(r"^x86_(16|32|64)-")


def tags(source: str) -> list[str] | None:
    """The tokens of the program's `// targets:` line, or None where it has none."""
    for line in source.splitlines()[:HEADER_LINES]:
        if line.startswith("// targets:"):
            return line.removeprefix("// targets:").split()
    return None


def applies(target: str, source: str) -> bool:
    listed = tags(source)
    if listed is None:
        return True
    family = FAMILY.match(target)
    names = {target, f"x86-m{family.group(1)}" if family else target}
    return any(token in names or (token == "dos" and DOS.match(target)) for token in listed)


def reason(source: str) -> str:
    listed = tags(source) or []
    return "n/a, DOS-only" if listed == ["dos"] else "n/a, targets: " + " ".join(listed)


def main(argv: list[str]) -> int:
    if len(argv) != 4 or argv[1] != "check":
        print("usage: corpus_targets.py check TARGET FILE.c", file=sys.stderr)
        return 2
    source = Path(argv[3]).read_text()
    if applies(argv[2], source):
        return 0
    print(reason(source))
    return 3


if __name__ == "__main__":
    sys.exit(main(sys.argv))
