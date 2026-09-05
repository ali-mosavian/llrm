#!/usr/bin/env python3
"""Run the whole flow over objects and say what each step did.

    tools/flow.py fixtures/omf/*.obj

parse -> raise -> passes -> lower -> allocate -> write, which is qbopt/flow.py.
Nothing here is the shipped path: rewrite.py still goes through wholeseg,
and this is how the two are compared while the seams move.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from collections import Counter

from qbopt import flow


def main(argv: list[str]) -> int:
    files = [Path(one) for one in argv[1:]] or sorted(Path("fixtures/omf").glob("*.obj"))
    written = spilled = 0
    was = now = 0
    why: Counter = Counter()
    for path in files:
        raw = path.read_bytes()
        try:
            out, reason = flow.run(raw)
        except Exception as error:
            why[f"{type(error).__name__}: {error}"] += 1
            continue
        if reason != "written":
            why[reason] += 1
            continue
        written += 1
        was += len(raw)
        now += len(out)
        if len(files) <= 20:
            print(f"  {path.stem:22} {len(raw):7} -> {len(out):7}  {len(out) - len(raw):+}")
    print(f"\n  {written} of {len(files)} written   {was} -> {now} bytes ({now - was:+})")
    for text, count in why.most_common(10):
        print(f"    {count:4}  {text}")
    return 0 if written == len(files) else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
