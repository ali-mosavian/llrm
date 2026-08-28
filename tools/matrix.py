"""
Every configuration, in parallel, as one table.

BC's three compilers do not emit the same code and a pass that is green on one
may not reach the others at all. The comparison operand order was read off
VBDOS /G3 alone and looked settled until the other three were checked; the same
rule applies to the tests.
"""

import sys
import argparse
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).resolve().parent))

import e2e
from configs import CONFIGS


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="matrix")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--prog")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--timeout", type=int, default=300)
    args = ap.parse_args(argv)

    tags = [t for t, c in CONFIGS.items() if c.available]
    if skipped := [t for t in CONFIGS if t not in tags]:
        print(f"no toolchain for {', '.join(skipped)}; see docs/testing.md")

    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        results = list(
            pool.map(
                lambda t: e2e.run(t, args.prog, dry_run=args.dry_run, timeout=args.timeout),
                tags,
            )
        )

    bad = 0
    for r in results:
        worst = next((v for v in r.verdicts if not v.ok), None)
        if worst:
            bad += 1
            print(f"  {r.tag:9} {worst.status:9} {worst.program}: {worst.detail}")
        else:
            print(f"  {r.tag:9} PASS      {len(r.verdicts)} programs")
    print(f"\n{len(results) - bad} of {len(results)} configurations pass")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
