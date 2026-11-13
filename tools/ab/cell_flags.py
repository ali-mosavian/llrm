#!/usr/bin/env python3
"""The flags a corpus program is built with in an A/B cell: the `-f*`, `-D*` and `-U*` words of the first variant of its
`// flags:` line. The level comes from the cell and the machine flags (`-m32`, `-march=`, `-mcmodel=`) from the target, so
neither is taken from the file. A target's own `// flags-FAMILY:` line (`flags-m68k: -mcmodel=medium`) is the one machine flag the
file does give, and only to that family (`corpus_targets.flags_for`): without it memcpy_runtime overflowed a pc-relative displacement on m68k
and its 8 cells came out EMPTY."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_targets  # noqa: E402

HEADER_LINES = 6


def cell_flags(source: str, target: str = "") -> list[str]:
    own: list[str] = []
    for line in source.splitlines()[:HEADER_LINES]:
        if line.startswith("// flags:"):
            first = line.removeprefix("// flags:").split("|")[0].split()
            own = [word for word in first if word.startswith(("-f", "-D", "-U"))]
            break
    return [*own, *corpus_targets.flags_for(target.split("-")[0], source)] if target else own


if __name__ == "__main__":
    # A missing file has no flags; the compile of it fails on its own.
    path = Path(sys.argv[1])
    target = sys.argv[2] if len(sys.argv) > 2 else ""
    print(" ".join(cell_flags(path.read_text(), target)) if path.is_file() else "")
