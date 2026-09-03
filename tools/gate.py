"""
The per-iteration gate: everything worth knowing in about a minute.

The full gate is four minutes of pytest and ten of matrix, which is too slow
to run between edits -- and a gate nobody runs is not one. This is the subset
that has actually caught things:

  the four test files that cover the passes and the allocator       ~11s
  the 485-object rebuild count, which layout breaks first          ~10s
  four programs through all twelve configurations                  ~50s

Four programs rather than forty, chosen for their shapes: hotlop folds,
pressx accumulates a chain, lngmix is longs and a runtime divide, nested is
loops inside loops. A pass that breaks one of those breaks something.

It is a filter, not the gate. `tools/matrix.py` and the full suite still run
at every phase boundary, and nothing is claimed green on this alone.
"""

import sys
import time
import argparse
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import e2e
from configs import CONFIGS
from matrix import MISCOMPILE

HERE = Path(__file__).resolve().parent.parent

# The pass and allocator tests. Not the whole suite: these are the files
# whose failures have meant something, and 2,556 of them run in eleven
# seconds.
TESTS = (
    "tests/test_transform.py",
    "tests/test_allocation.py",
    "tests/test_regalloc.py",
    "tests/test_lir.py",
    "tests/test_consts.py",
    "tests/test_runtime.py",
    # These two because leaving them out made the gate say clean while the
    # full suite had 73 failures: they are what checks that every byte and
    # every fixup still has a home, and a pass that rewrites an operand
    # breaks that before it breaks anything else.
    "tests/test_wholeseg.py",
    "tests/test_layout.py",
    "tests/test_rule5.py",
)

# Five shapes. harr is here because a change that only touched harr hung it
# on four configurations and the gate said clean: whatever is being worked
# on belongs in this list.
PROGRAMS = ("hotlop", "pressx", "lngmix", "nested", "harr")


def _tests() -> tuple[bool, str]:
    got = subprocess.run(
        [sys.executable, "-m", "pytest", "-q", *TESTS],
        cwd=HERE,
        capture_output=True,
        text=True,
    )
    tail = [one for one in got.stdout.splitlines() if one.strip()]
    return got.returncode == 0, tail[-1] if tail else "no output"


def _rebuild() -> tuple[bool, str]:
    """Every object still emitting from MIR rather than falling back.

    layout.py refuses a body it cannot account for every byte of, and that
    is the first thing a pass that deletes or moves an operation breaks.
    """
    from qbopt import wholeseg

    objects = sorted((HERE / "fixtures" / "omf").glob("*.obj"))
    refused = [
        one.name
        for one in objects
        if wholeseg.rebuilt(one.read_bytes())[1] != wholeseg.REBUILT
    ]
    if refused:
        return False, f"{len(objects) - len(refused)} of {len(objects)}: {refused[:3]}"
    return True, f"{len(objects)} of {len(objects)}"


def _programs(jobs: int, timeout: int) -> tuple[bool, str]:
    tags = [tag for tag, one in CONFIGS.items() if one.available]
    if not tags:
        return False, "no toolchain; see docs/testing.md"
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        results = list(
            pool.map(
                lambda tag: e2e.run(tag, names=list(PROGRAMS), timeout=timeout, work=HERE / "build" / f"gate-{tag}"),
                tags,
            )
        )
    wrong, stuck = [], []
    for one in results:
        worst = next((v for v in one.verdicts if not v.ok), None)
        if worst is None:
            continue
        (wrong if worst.status in MISCOMPILE else stuck).append(f"{one.tag} {worst.program}")
    if wrong:
        return False, f"MISCOMPILED {', '.join(wrong)}"
    if stuck:
        return False, f"no answer from {', '.join(stuck)} -- rerun"
    return True, f"{len(tags)} configurations x {len(PROGRAMS)} programs"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="gate")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--timeout", type=int, default=300)
    ap.add_argument("--no-programs", action="store_false", dest="programs", help="tests and rebuild only")
    args = ap.parse_args(argv)

    steps = [("tests", _tests), ("rebuild", _rebuild)]
    if args.programs:
        steps.append(("programs", lambda: _programs(args.jobs, args.timeout)))

    bad = 0
    for name, step in steps:
        began = time.monotonic()
        ok, detail = step()
        took = time.monotonic() - began
        print(f"  {'ok  ' if ok else 'FAIL'} {name:9s} {took:5.1f}s  {detail}")
        bad += not ok
    print("\ngate clean" if not bad else f"\n{bad} of {len(steps)} failed")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
