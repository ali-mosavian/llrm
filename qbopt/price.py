"""
What the pass costs, in cycles rather than bytes.

Bytes are a proxy and not the thing: a shorter sequence with a dependency chain
through one register can be slower than a longer one, and putting the high half
back sits at the end of a chain. qbopt/cycles knows the difference, so ask it.

    uv run python -m qbopt.price FILE.OBJ

Every figure is a published latency rather than a measurement, so what comes out
is a ranking. DOSBox charges per instruction and models no latency, so it
answers only for an in-order machine; these two disagree on purpose.

**This prices what is in the object and nothing else.** BC's side of an absorbed
call is `push / push / call`, three instructions, and the routine behind the
call is not in this module and is not counted. So an absorbed call reads as a
large loss here and is not one: an absorbed divide replaces a far call into a
routine that normalises its operands one bit at a time, up to fifteen passes of
twelve instructions. `python -m qbopt.cycles.cycles` holds those bodies and
prices them; this cannot.

**Absorbing every call to a routine does not remove it from the linked
program.** Measured on `bench/nbody.bas`: `B$MUI4`/`B$DVI4`/`B$CPI4`/`B$RMI4`
share one 262-byte runtime module (`runtime/rt/helpi4.asm`); the rewrite
drops every one of the 21 calls into the first three, yet that module's own
code is byte-identical and present in both `BASE.EXE` and `OPT.EXE` -- linked
in either way by something else in the runtime, not by this program's own
calls. The cycle price this module reports for an absorbed call is real, but
it is not a linked-program-size saving, and nothing here or in `cycles.py`
prices a `.LIB`.
"""

import sys
import argparse
from pathlib import Path
from dataclasses import dataclass

from qbopt import omf
from qbopt.cycles import ARCHS
from qbopt.rewrite import plan
from qbopt.cycles import report

# what the two models mean, in the order cycles.report gives them
STANDING = 0, "standing alone -- the sequence waits on its own chain"
BACK_TO_BACK = 1, "back to back -- the machine has other work to overlap"


@dataclass(frozen=True, slots=True)
class Priced:
    at: int
    before_instructions: int
    after_instructions: int
    before: tuple[list[float], ...]
    after: tuple[list[float], ...]


def priced(records: list[omf.Record]) -> list[Priced]:
    out = []
    for one in plan(records):
        region = one.region
        if not region.taken or region.after is None:
            continue
        _n, before_count, before_cost, _d = report("before", region.before)
        _n, after_count, after_cost, _d = report("after", region.after)
        out.append(Priced(region.at, before_count, after_count, before_cost, after_cost))
    return out


def show(rows: list[Priced]) -> None:
    if not rows:
        print("no regions taken")
        return
    for which, title in (STANDING, BACK_TO_BACK):
        before = [sum(row.before[which][n] for row in rows) for n in range(len(ARCHS))]
        after = [sum(row.after[which][n] for row in rows) for n in range(len(ARCHS))]
        print()
        print(title)
        print(f"{'':16}" + "".join(f"{arch:>8}" for arch in ARCHS))
        print(f"{'BC emits':16}" + "".join(f"{x:>8g}" for x in before))
        print(f"{'rewritten':16}" + "".join(f"{x:>8g}" for x in after))
        print(
            f"{'speedup':16}"
            + "".join(f"{b / a:>7.2f}x" if a else f"{'-':>8}" for b, a in zip(before, after, strict=True))
        )

    kept = sum(row.before_instructions for row in rows)
    now = sum(row.after_instructions for row in rows)
    print()
    print(f"{len(rows)} regions, instructions {kept} -> {now}, {kept / now:.2f}x")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="qbopt.price")
    ap.add_argument("object", type=Path)
    args = ap.parse_args(argv)
    show(priced(omf.read(args.object)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
