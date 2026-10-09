"""The runtime's size, and each program's linked size against BCOM45's.

    python tools/dosbatch/qbsizes.py [--work DIR] [source.bas ...]    (the 25 bench programs by default)
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import qbruntime  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, default=None)
    parser.add_argument("sources", nargs="*", type=Path)
    args = parser.parse_args()
    work = args.work or Path(tempfile.mkdtemp(prefix="qbsizes-"))
    work.mkdir(parents=True, exist_ok=True)
    sources = args.sources or qbruntime.milestone_sources()
    objects = {}
    for source in sources:
        pair = (work / f"{source.stem}.qb45.obj", work / f"{source.stem}.llrm.obj")
        for runtime, obj in zip(("qb45", "llrm"), pair):
            if reason := qbruntime.compile_basic(source, obj, runtime):
                raise SystemExit(f"{source}: {reason}")
        objects[source.stem] = pair
    archive, _ = qbruntime.build(work / "archive")
    found = qbruntime.differential_batch(objects, archive, work / "differential")
    print(f"LLRMQB.LIB {archive.stat().st_size} bytes")
    print(f"{'program':<14}{'BCOM45':>9}{'LLRMQB':>9}{'ratio':>8}  output")
    for name, result in found.items():
        want, got = result.sizes
        same = "identical" if result.candidate.status == "ok" and not result.difference else "DIFFERENT"
        print(f"{name:<14}{want:>9}{got:>9}{got / want if want else 0:>8.2f}  {same}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
