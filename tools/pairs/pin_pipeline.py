#!/usr/bin/env python3
"""pin_pipeline.py [OUT_DIR]: the pin's `-print-pipeline-passes` at -O1, -O2, -O3 and -Os, one fixture per level.

Run through tools/pairs/pin.py with the m32 target (i386, -march=i486, no SSE), never the host's; each file starts with `;` lines naming
the command, the target and the md5 of the `opt` that ran, so the fixture can be checked against the pin it came from. Output goes to
tests/fixtures/pin-pipeline by default. The Rust test `pin_pipeline_slots` reads them.
"""
from __future__ import annotations

import hashlib
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pin  # noqa: E402

# The pin has no -Os pipeline ("use O2 in conjunction with the optsize attribute"): -Os is default<O2> over functions marked optsize,
# which changes what the passes decide and no slot of the printed pipeline.
LEVELS = {"O1": ("O1", False), "O2": ("O2", False), "O3": ("O3", False), "Os": ("O2", True)}
IR = "define void @f() {\n  ret void\n}\n"


def main(argv: list[str]) -> int:
    out = Path(argv[0]) if argv else HERE.parents[1] / "tests" / "fixtures" / "pin-pipeline"
    out.mkdir(parents=True, exist_ok=True)
    opt = pin.PINNED_BIN / "opt"
    md5 = hashlib.md5(opt.read_bytes()).hexdigest()
    for level, (pipeline, optsize) in LEVELS.items():
        of = pin.target("m32", optsize=optsize)
        passes = f"default<{pipeline}>"
        done = pin.opt(passes, IR, of, extra=["-print-pipeline-passes"])
        if done.returncode != 0:
            sys.stderr.write(done.stderr)
            return 1
        body = [line for line in done.stdout.splitlines() if not line.startswith(";")]
        header = [
            f"; pin: LLVM d1106deb71cc, opt md5 {md5}",
            f"; command: tools/pairs/pin_pipeline.py (opt -passes={passes} -print-pipeline-passes, {pin.header(of)})",
        ]
        (out / f"{level}.txt").write_text("\n".join(header + body) + "\n")
        print(level, len(body), "lines")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
