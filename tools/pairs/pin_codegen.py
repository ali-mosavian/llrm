#!/usr/bin/env python3
"""The pin's codegen pipeline (`llc -debug-pass=Structure`) at -O1, -O2, -O3 and -Os for m32 and m16, into
tests/fixtures/pin-codegen/<mode>-<level>.txt. Every run is given an explicit Target (pin.py); -Os is -O2 on a module whose
functions carry optsize, as llc has no -Os. The pipeline is the same at every level (-O0 alone differs): the passes decide
inside, so the rows of the slot table carry the level in their ablation, not in their position."""
from pathlib import Path
import sys

import pin

OUT = Path(__file__).resolve().parents[2] / "tests/fixtures/pin-codegen"
SRC = "define i32 @f(i32 %a, i32 %b) {\n  %c = add i32 %a, %b\n  ret i32 %c\n}\n"


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    for mode in ("m32", "m16"):
        for level in ("O1", "O2", "O3", "Os"):
            of = pin.target(mode, optsize=level == "Os")
            done = pin.llc([f"-O{'2' if level == 'Os' else level[1]}", "-debug-pass=Structure", "-filetype=asm", "-o", "/dev/null"], SRC, of)
            if done.returncode:
                sys.exit(f"{mode} {level}: {done.stderr[:300]}")
            lines = done.stderr.splitlines()[1:]  # the first line is the argument list
            (OUT / f"{mode}-{level}.txt").write_text(f"; {pin.header(of)}\n" + "\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
