"""
What the pass makes of a pile of objects.

    uv run python tools/census.py fixtures/omf
    uv run python tools/census.py build/qrender

The point is the refusals as much as the takings: a reason that dominates is
where the next piece of work is, and a module that cannot be mapped at all is
a shape the analysis has not met.
"""

import sys
import argparse
from pathlib import Path
from collections import Counter

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.frontend.blocks import code_map
from qbopt.rewrite import rewrite


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="census")
    ap.add_argument("where", type=Path, nargs="?", default=Path("fixtures/omf"))
    args = ap.parse_args(argv)

    objects = sorted(p for p in args.where.iterdir() if p.suffix.lower() == ".obj")
    why: Counter[str] = Counter()
    mapped = unmapped = taken = total = before = after = code = 0

    for path in objects:
        records = omf.read(path)
        found = module.of(records)
        if found is None:
            continue
        code += found.end
        if isinstance(code_map(found), str):
            unmapped += 1
            continue
        mapped += 1
        _, regions = rewrite(path.read_bytes(), dry_run=False)
        for region in regions:
            total += 1
            if region.taken and region.after is not None:
                taken += 1
                before += region.end - region.at
                after += len(region.after) // 2
            else:
                why[(region.reason or "?").split(" 0x")[0]] += 1

    saved = f"{100 * (before - after) // before} per cent smaller" if before else "nothing taken"
    print(f"{len(objects)} objects, {code} bytes of code: {mapped} mapped, {unmapped} refused")
    print(f"{taken} of {total} regions taken, {before} -> {after} bytes, {saved}")
    for reason, count in why.most_common():
        print(f"  {count:5}  {reason}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
