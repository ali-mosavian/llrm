#!/usr/bin/env python3
"""Names each `def : Pat<...>;` of a .td that gives llvm-tblgen -gen-dag-isel no matcher entry.

llvm-tblgen drops such a Pat without a word (an untyped nested result, for one). The table is made once whole and
once per Pat with that Pat blanked; the same table means the Pat did nothing.

  check_pats.py TD [-I DIR]... -- TBLGEN [ARGS...]

TBLGEN and its ARGS are the command regen-target.sh uses, `-gen-dag-isel` and the .td not included. Only a Pat
written out as `def : Pat<...>;` in TD itself is tried: one made by a multiclass or another file is not.
Exits 1 with one `file:line: ...` per Pat found.
"""

import re
import subprocess
import sys
import tempfile
from pathlib import Path


def pats(text):
    """(start, end) offsets of each `def : Pat<...>;`, its angle brackets and parentheses balanced."""
    found = []
    for start in (m.start() for m in re.finditer(r"def\s*:\s*Pat\s*<", text)):
        depth = 0
        for at in range(text.index("<", start), len(text)):
            char = text[at]
            if char == "<":
                depth += 1
            elif char == ">":
                depth -= 1
                if depth == 0:
                    end = text.index(";", at) + 1
                    found.append((start, end))
                    break
    return found


def table(command, td, out_dir):
    out = Path(out_dir) / "GenDAGISel.inc"
    done = subprocess.run([*command, "-gen-dag-isel", str(td), "-o", str(out)], capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"{td}: tblgen failed:\n{done.stderr}")
    return out.read_text()


def check(td, includes, command):
    """The Pats of `td` that change nothing, as `file:line: ...` lines."""
    td = Path(td)
    text = td.read_text()
    flags = [arg for include in includes for arg in ("-I", str(include))]
    with tempfile.TemporaryDirectory() as work:
        whole = table([*command, "-I", str(td.parent), *flags], td, work)
        found = []
        for start, end in pats(text):
            blank = text[:start] + re.sub(r"[^\n]", " ", text[start:end]) + text[end:]
            trial = Path(work) / td.name
            trial.write_text(blank)
            if table([*command, "-I", str(td.parent), *flags], trial, work) == whole:
                line = text.count("\n", 0, start) + 1
                head = " ".join(text[start:end].split())[:100]
                found.append(f"{td}:{line}: this Pat gives no matcher entry (tblgen dropped it): {head}")
    return found


def main(argv):
    if "--" not in argv:
        sys.exit(__doc__)
    split = argv.index("--")
    args, command = argv[:split], argv[split + 1:]
    includes = [args[i + 1] for i, arg in enumerate(args) if arg == "-I"]
    found = check(args[0], includes, command)
    if found:
        print("\n".join(found))
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
