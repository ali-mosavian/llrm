#!/usr/bin/env python3
"""The flags a corpus program is built with in an A/B cell: the `-f*`, `-D*` and `-U*` words of the first variant of its
`// flags:` line. The level comes from the cell and the machine flags (`-m32`, `-march=`, `-mcmodel=`) from the target, so
neither is taken from the file."""
import sys
from pathlib import Path

HEADER_LINES = 6


def cell_flags(source: str) -> list[str]:
    for line in source.splitlines()[:HEADER_LINES]:
        if line.startswith("// flags:"):
            first = line.removeprefix("// flags:").split("|")[0].split()
            return [word for word in first if word.startswith(("-f", "-D", "-U"))]
    return []


if __name__ == "__main__":
    # A missing file has no flags; the compile of it fails on its own.
    path = Path(sys.argv[1])
    print(" ".join(cell_flags(path.read_text())) if path.is_file() else "")
