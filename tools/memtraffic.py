"""
What BC leaves on the table: memory traffic a register would have made
unnecessary.

BC targets an 8086 and keeps almost everything in memory. It writes a named
variable and reads it back a few instructions later; it stores a value it is
still holding in a register purely to push it for a call. Neither is a bug --
it has no register allocator -- and both are exactly what one would remove.
This counts them, so the size of that prize is a measurement rather than an
impression, and so the number moves visibly once something starts taking it.

    uv run python tools/memtraffic.py fixtures/omf build/bench
    uv run python tools/memtraffic.py --per-object build/bench

The analysis itself is qbopt/memory.py -- forward availability for a load
whose value was already in hand, backward liveness for a store nothing reads.
This is only the reporting, and it splits by loop nesting depth because the
totals alone mislead: an opportunity in straight-line setup code is worth
taking once, and the same one three loops deep is worth taking every trip.
qbopt/loops.py's own caveat applies -- depth is within a body, so this ranks
a kernel against its own neighbours and not against another procedure's.
"""

import sys
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt import omf
from qbopt import loops
from qbopt import blocks
from qbopt import memory
from qbopt import module

ROOT = Path(__file__).resolve().parents[1]


def measure(path: Path) -> dict[int, tuple[int, int]] | None:
    """(redundant loads, dead stores) per loop nesting depth."""
    found = module.of(omf.parse(path.read_bytes()))
    if found is None:
        return None
    mapped = blocks.code_map(found)
    if isinstance(mapped, str):
        return None
    partitioned = blocks.partition(found, mapped)
    if not partitioned:
        return None

    where = (partitioned, found.resolve, found.calls, found.dgroup)
    reloads = memory.redundant_loads(*where)
    stores = memory.dead_stores(*where)
    nesting = loops.depth(partitioned)

    by_depth: dict[int, tuple[int, int]] = {}
    for block in partitioned:
        at_depth = nesting.get(block.at, 0)
        was = by_depth.get(at_depth, (0, 0))
        by_depth[at_depth] = (
            was[0] + len(reloads.get(block.at, ())),
            was[1] + len(stores.get(block.at, ())),
        )
    return by_depth


def objects(where: Path) -> list[Path]:
    if where.is_file():
        return [where]
    return sorted(where.rglob("*.obj")) + sorted(where.rglob("*.OBJ"))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="memtraffic")
    ap.add_argument("where", type=Path, nargs="+")
    ap.add_argument("--per-object", action="store_true", help="one line per object, not just the total")
    args = ap.parse_args(argv)

    paths = [p for w in args.where for p in objects(w)]
    totals: dict[int, tuple[int, int]] = {}
    skipped = 0

    for path in paths:
        found = measure(path)
        if found is None:
            skipped += 1
            continue
        for at_depth, got in found.items():
            was = totals.get(at_depth, (0, 0))
            totals[at_depth] = (was[0] + got[0], was[1] + got[1])
        if args.per_object and any(any(v) for v in found.values()):
            loads = sum(v[0] for v in found.values())
            stores = sum(v[1] for v in found.values())
            print(f"  {path.name:<28} loads {loads:>4}  stores {stores:>4}")

    print(f"\n{len(paths) - skipped} objects")
    print(f"  {'loop depth':<12} {'redundant loads':>16} {'dead stores':>13}")
    for at_depth in sorted(totals):
        if not any(totals[at_depth]):
            continue  # RESUME's own dispatch reaches depth 28 and carries nothing
        loads, stores = totals[at_depth]
        print(f"  {at_depth:<12} {loads:>16} {stores:>13}")
    print(f"  {'total':<12} {sum(v[0] for v in totals.values()):>16} {sum(v[1] for v in totals.values()):>13}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
