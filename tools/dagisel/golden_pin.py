#!/usr/bin/env python3
"""Regenerates `golden/NAME.pin.txt` from `golden/NAME.ll`: the pin's machine opcodes per function.

    tools/dagisel/golden_pin.py x87        # in crates/codegen/llrm-codegen/src/backend/selectiondag/golden

One line per function, `name OPCODE:count,...`, sorted, bookkeeping opcodes left out (the shadow tool's rule).
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "pairs"))
import pin  # noqa: E402
import select_shadow as shadow  # noqa: E402

GOLDEN = Path(__file__).resolve().parents[2] / "crates/codegen/llrm-codegen/src/backend/selectiondag/golden"


def render(ir: str) -> str:
    done = pin.llc(["-O2", "-stop-after=finalize-isel", "-o", "-"], ir, pin.target("m32"))
    if done.returncode != 0:
        raise SystemExit(done.stderr)
    lines = []
    for name, counts in shadow.pin_opcodes(done.stdout).items():
        items = ",".join(f"{k}:{v}" for k, v in sorted(counts.items()) if k not in shadow.BOOKKEEPING)
        lines.append(f"{name} {items}\n")
    return "".join(lines)


def main() -> None:
    name = sys.argv[1]
    (GOLDEN / f"{name}.pin.txt").write_text(render((GOLDEN / f"{name}.ll").read_text()))


if __name__ == "__main__":
    main()
