"""
What BC leaves on the table, counted rather than assumed.

This exists because the roadmap said for months that the classic passes
measure empty on BC's output, and that was the metric rather than BC. Every
measurement behind it asked a question shaped so that BC's own style could
not answer it: CSE over SSA values, when BC never recomputes and always
reloads; a redundant load defined as one into the register that already
holds the cell, when BC reloads into a different one.

So the questions here are deliberately crude and about *cells*, not values:
what is loaded that was just written, what is loaded twice, what is stored
twice with nothing reading it in between. A crude count that is right is
worth more than a precise one that is measuring the wrong thing.

Block-scoped, and that is a floor rather than an answer: BC's loop counter
round-trips through memory every iteration and the reload arrives over the
back-edge, which nothing here sees.
"""

import sys
import argparse
from pathlib import Path
from collections import Counter

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from qbopt import mir
from qbopt import omf
from qbopt import module
from qbopt import blocks as split
from qbopt.module import Space
from qbopt.blocks import code_map


def named(ref) -> bool:
    """A cell this can name. A stack slot is a depth from the top of its own
    block, and an address of None aliases everything."""
    return ref.addr is not None and ref.addr.space is not Space.STACK


def counted(paths: list[Path]) -> Counter:
    found: Counter = Counter()
    for path in paths:
        module_ = module.of(omf.parse(path.read_bytes()))
        if module_ is None:
            continue
        mapped = code_map(module_)
        if isinstance(mapped, str):
            continue
        for _name, body in mir.bodies(module_, split.partition(module_, mapped)):
            for block in body.blocks:
                written: dict[str, int] = {}
                read: dict[str, int] = {}
                for op in block.ops:
                    # A call may write any cell and a barrier's addresses are
                    # its own, so both end what is known here.
                    if op.barrier or op.at in module_.calls:
                        written.clear()
                        read.clear()
                        continue
                    for ref in (one for one in op.loads if named(one)):
                        cell = str(ref.addr)
                        if cell in written:
                            found["load of a cell just written"] += 1
                        elif cell in read:
                            found["load of a cell already loaded"] += 1
                        read[cell] = op.at
                    for ref in (one for one in op.stores if named(one)):
                        cell = str(ref.addr)
                        if cell in written:
                            found["store over a store nothing read"] += 1
                        written[cell] = op.at
                        read.pop(cell, None)
    return found


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="opportunity")
    ap.add_argument("objects", nargs="*", type=Path)
    args = ap.parse_args(argv)
    paths = args.objects or sorted(Path("fixtures/omf").glob("*.obj"))
    found = counted(paths)
    for name, count in sorted(found.items(), key=lambda one: -one[1]):
        print(f"  {count:6d}  {name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
