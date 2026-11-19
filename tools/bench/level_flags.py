#!/usr/bin/env python3
"""level_flags.py SRC LEVEL: the m64 flags of one matrix cell and where they came from: `header` or `LEVELS`.

A `// flags:` alternative that names LEVEL and no -m32/-m16 is the header's own cell. Without one, the cell comes from LEVELS: LEVEL plus the
`-f*`, `-D*`, `-U*` words of the first alternative (tools/ab/cell_flags.py), never the header's -O2. The header is also the DOS gate's input, so it
is not grown for m64 levels. Output: `SOURCE<TAB>FLAGS`."""
import re
import sys
from pathlib import Path

HEADER_LINES = 6
MACHINE = re.compile(r"^(-m32|-m16|-march=\S+)$")
KEPT = ("-f", "-D", "-U")


def alternatives(text: str) -> list[list[str]] | None:
    for line in text.splitlines()[:HEADER_LINES]:
        if line.startswith("// flags:"):
            return [alt.split() for alt in line.removeprefix("// flags:").split("|")]
    return None


def cell(text: str, level: str) -> tuple[str, list[str]]:
    level = "-" + level.lstrip("-")
    alts = alternatives(text)
    if alts is None:
        return "LEVELS", []
    for alt in alts:
        if level in alt and "-m32" not in alt and "-m16" not in alt:
            return "header", [w for w in alt if w != level and not MACHINE.match(w)]
    return "LEVELS", [w for w in alts[0] if w.startswith(KEPT)]


if __name__ == "__main__":
    source, flags = cell(Path(sys.argv[1]).read_text(), sys.argv[2])
    print(source, " ".join(flags), sep="\t")
