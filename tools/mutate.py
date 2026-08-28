"""
Put each bug back, and check something notices.

A test suite that has never been watched failing proves nothing about the bugs
it was written for. This applies a named one-line change to a copy of the tree,
runs the tests there, and reports which mutations survived. A mutation nothing
catches is a hole, and the run fails on one.

Confirming the mutation applied is half of it, and the half usually skipped:
without it a typo in the pattern reads as "the tests caught nothing", which
looks the same as a mutation that could not be made.

    uv run python tools/mutate.py
    uv run python tools/mutate.py --only checksum
"""

import sys
import shutil
import argparse
import subprocess
from pathlib import Path
from dataclasses import dataclass
from tempfile import TemporaryDirectory
from concurrent.futures import ThreadPoolExecutor

ROOT = Path(__file__).resolve().parents[1]
COPIED = ("qbopt", "tests", "tools", "fixtures", "suite", "pyproject.toml")


@dataclass(frozen=True, slots=True)
class Mutation:
    name: str
    file: str
    before: str
    after: str
    bug: str


MUTATIONS = (
    Mutation(
        "checksum",
        "qbopt/omf.py",
        "return head + body + bytes([(-sum(head) - sum(body)) & 0xFF])",
        "return head + body + bytes([0])",
        "a zero checksum, which many tools write and BC does not",
    ),
    Mutation(
        "frame-thread-index",
        "qbopt/omf.py",
        "    if method < 3:",
        "    if method & 3 < 3:",
        "frame methods 4 and 5 read an index they do not carry",
    ),
    Mutation(
        "displacement-dropped",
        "qbopt/omf.py",
        '                disp, disp_pos = struct.unpack_from("<H", body, at)[0], at',
        "                disp, disp_pos = 0, at",
        "the target displacement, which holds every static address",
    ),
    Mutation(
        "ledata-first-write-wins",
        "qbopt/omf.py",
        "    for _record, index, offset, payload in ledata(records):",
        "    for _record, index, offset, payload in reversed(ledata(records)):",
        "BC's backpatch records overwritten by the earlier ones",
    ),
    Mutation(
        "shift-boundary",
        "qbopt/relocate.py",
        "            if offset >= edit.hi:",
        "            if offset > edit.hi:",
        "an offset exactly at a region's end is treated as inside it",
    ),
    Mutation(
        "branch-from-start",
        "qbopt/relocate.py",
        "    return shift.at(branch.target) - shift.at(branch.end)",
        "    return shift.at(branch.target) - shift.at(branch.at)",
        "a branch mapped from its own start rather than its end",
    ),
    Mutation(
        "rel8-truncated",
        "qbopt/relocate.py",
        "        if not reaches(branch, moved):",
        "        if False:",
        "a rel8 that no longer reaches, truncated instead of refused",
    ),
    Mutation(
        "target-not-shifted",
        "qbopt/relocate.py",
        "                disp = shift.at(fixup.disp) if into_code else None",
        "                disp = fixup.disp if into_code else None",
        "a fixup's offset moved but not what it points at",
    ),
    Mutation(
        "divergence-gate",
        "qbopt/flags.py",
        "DIVERGENT = Flag.ZF | Flag.PF | Flag.AF",
        "DIVERGENT = Flag.NONE",
        "the flag gate opened",
    ),
    Mutation(
        "unmodelled-reads-nothing",
        "qbopt/flags.py",
        "        case _:\n            return Effect(ALL, Flag.NONE)",
        "        case _:\n            return Effect(Flag.NONE, Flag.NONE)",
        "an instruction nothing models assumed harmless",
    ),
    Mutation(
        "leaves-not-conservative",
        "qbopt/flags.py",
        "            out = ALL if block.leaves else Flag.NONE",
        "            out = Flag.NONE",
        "flags assumed dead past an edge nothing can see",
    ),
    Mutation(
        "call-ends-a-block",
        "qbopt/blocks.py",
        "        case opcode if opcode in RETURNS:",
        "        case 0x9A | 0xE8:\n            return Ends.RETURN\n        case opcode if opcode in RETURNS:",
        "a call treated as the end of a block, which it is not",
    ),
    Mutation(
        "relocated-operand-not-zero",
        "qbopt/lift.py",
        '            return b"\\x00" * value.dlen',
        '            return struct.pack("<H", value.mem.disp)',
        "a relocated operand emitted with its address in the code, which LINK adds to",
    ),
)


@dataclass(frozen=True, slots=True)
class Outcome:
    mutation: Mutation
    applied: bool
    caught: bool
    detail: str


def run_one(mutation: Mutation, into: Path, marker: str) -> Outcome:
    tree = into / mutation.name
    tree.mkdir(parents=True)
    for name in COPIED:
        source = ROOT / name
        if source.is_dir():
            shutil.copytree(source, tree / name, symlinks=True)
        else:
            shutil.copy(source, tree / name)

    target = tree / mutation.file
    text = target.read_text()
    if text.count(mutation.before) != 1:
        return Outcome(mutation, False, False, f"pattern appears {text.count(mutation.before)} times")
    target.write_text(text.replace(mutation.before, mutation.after))

    done = subprocess.run(
        [sys.executable, "-m", "pytest", "-x", "-q", "-p", "no:cacheprovider", "-m", marker],
        cwd=tree,
        capture_output=True,
        check=False,
    )
    tail = done.stdout.decode().strip().splitlines()
    return Outcome(mutation, True, done.returncode != 0, tail[-1] if tail else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="mutate")
    ap.add_argument("--only", action="append", choices=[m.name for m in MUTATIONS])
    ap.add_argument("--marker", default="not e2e", help="which tier to run against each mutation")
    ap.add_argument("--jobs", type=int, default=4)
    args = ap.parse_args(argv)

    wanted = [m for m in MUTATIONS if not args.only or m.name in args.only]
    with TemporaryDirectory() as temporary:
        into = Path(temporary)
        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            outcomes = list(pool.map(lambda m: run_one(m, into, args.marker), wanted))

    survived = []
    for outcome in outcomes:
        if not outcome.applied:
            state = "NOT APPLIED"
        elif outcome.caught:
            state = "caught"
        else:
            state = "SURVIVED"
        print(f"  {state:12} {outcome.mutation.name:26} {outcome.mutation.bug}")
        if not outcome.applied or not outcome.caught:
            survived.append(outcome)

    print(f"\n{len(outcomes) - len(survived)} of {len(outcomes)} mutations caught")
    for outcome in survived:
        print(f"  {outcome.mutation.name}: {outcome.detail}")
    return 1 if survived else 0


if __name__ == "__main__":
    sys.exit(main())
