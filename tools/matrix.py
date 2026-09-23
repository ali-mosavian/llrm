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

# Statuses where the rewritten program ran and printed the wrong thing. Every
# other failure -- a timeout, a compiler or linker error, the pass raising --
# means no answer came back, which is not the same finding and must not be
# counted as one.
MISCOMPILE = frozenset({"DIFF", "BASEDIFF"})


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="matrix")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--prog")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--timeout", type=int, default=300)
    ap.add_argument(
        "--no-absorb-calls",
        action="store_true",
        help="leave the arithmetic calls to the MIR tower instead of calls.py",
    )
    ap.add_argument("--rewriter", choices=e2e.REWRITERS, default="python", help="python -m qbopt.rewrite or llrm-omf")
    args = ap.parse_args(argv)

    tags = [t for t, c in CONFIGS.items() if c.available]
    if skipped := [t for t in CONFIGS if t not in tags]:
        print(f"no toolchain for {', '.join(skipped)}; see docs/testing.md")

    # --no-absorb-calls is the M5 comparison: calls.py absorbs every
    # arithmetic call before a body reaches the MIR tower, so the MIR
    # emitter is unreachable while the machine arm is on and no
    # configuration here exercises it.
    options = [flag for flag, on in (("--dry-run", args.dry_run), ("--no-absorb-calls", args.no_absorb_calls)) if on]
    command = e2e.rewriter_command(args.rewriter)
    print(f"rewriter: {' '.join(command)}")

    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        results = list(
            pool.map(
                lambda t: e2e.run(t, args.prog, timeout=args.timeout, transform=e2e.driver(command, CONFIGS[t], *options)),
                tags,
            )
        )

    wrong: list[str] = []  # the pass changed what the program computes
    stuck: list[str] = []  # no answer came back; says nothing about the pass
    for r in results:
        worst = next((v for v in r.verdicts if not v.ok), None)
        if worst is None:
            print(f"  {r.tag:9} PASS      {len(r.verdicts)} programs")
            continue
        (wrong if worst.status in MISCOMPILE else stuck).append(r.tag)
        print(f"  {r.tag:9} {worst.status:9} {worst.program}: {worst.detail}")

    # Two failures that look identical in a count and mean opposite things:
    # a wrong answer is this pass miscompiling, and no answer is the harness
    # or the machine. One matrix run came back "11 of 12" with nothing in
    # that line to say which, and three runs since have been clean -- so the
    # summary now says, and never reports a clean pass when anything is
    # unresolved.
    passed = len(results) - len(wrong) - len(stuck)
    print(f"\n{passed} of {len(results)} configurations pass")
    if wrong:
        print(f"  MISCOMPILED  {', '.join(wrong)} -- the rewrite changed what the program prints")
    if stuck:
        print(f"  NO ANSWER    {', '.join(stuck)} -- nothing was proved either way; rerun these")
    return 1 if wrong or stuck else 0


if __name__ == "__main__":
    sys.exit(main())
