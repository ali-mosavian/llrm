"""
Whether the corpus partitions completely into main bodies and procedures.

    uv run python tools/extent_census.py fixtures/omf

The point is the leftover as much as the count: a byte range nothing
accounts for names exactly where the next piece of work is.
"""

import sys
import argparse
from pathlib import Path
from collections import Counter

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt.objectfile import module
from qbopt.frontend.extent import BodyKind
from qbopt.frontend.extent import partition


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="extent_census")
    ap.add_argument("where", type=Path, nargs="?", default=Path("fixtures/omf"))
    args = ap.parse_args(argv)

    objects = sorted(p for p in args.where.iterdir() if p.suffix.lower() == ".obj")
    complete = incomplete = refused = 0
    procedures = event_stubs = 0
    leftover: Counter[str] = Counter()

    for path in objects:
        found = module.load(path)
        if found is None:
            refused += 1
            print(f"{path.name}: no code segment")
            continue
        found_partition = partition(found)
        if isinstance(found_partition, str):
            refused += 1
            print(f"{path.name}: {found_partition}")
            continue
        procedures += sum(1 for b in found_partition.bodies if b.kind is BodyKind.PROCEDURE)
        event_stubs += sum(1 for b in found_partition.bodies if b.kind is BodyKind.EVENT_STUB)
        if found_partition.complete:
            complete += 1
        else:
            incomplete += 1
            for lo, hi in found_partition.unexplained:
                print(f"{path.name}: {lo:#06x}-{hi:#06x} unexplained")
                leftover["unexplained"] += 1
            for lo, hi in found_partition.conflicts:
                print(f"{path.name}: {lo:#06x}-{hi:#06x} claimed by more than one body")
                leftover["conflict"] += 1

    print()
    print(f"{len(objects)} objects: {complete} partition completely, {incomplete} incomplete, {refused} refused")
    print(f"{procedures} procedure bodies, {event_stubs} event-poll stubs found")
    for reason, count in leftover.most_common():
        print(f"  {count:5}  {reason}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
