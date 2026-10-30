#!/usr/bin/env python3
"""The objects leg of the gate: the merge-base's objects against this build's, over every target and debug leg.

    tools/pairs/objsweep_gate.py

Two `llrm-cc` builds (the merge-base's with LLRM_MEASURE_REF, origin/main unless named, tools/measure.py's cached `dist`; this tree's), `objsweep.py` over m16, m32 and m64, plain and
`--debug`, O2 and Os. Each leg must change as many objects as `objsweep_expected.toml` says (none where it says nothing): a PR that moves
bytes on purpose lists the count and why, and the file is empty again once the base has the change. A leg that compiled too little
proves nothing and fails, and a control (the same program at O2 and at Os must differ) shows the comparison sees a change.
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "tools"))

LEGS = [(f"{target}{suffix}", target, debug) for target in ("m32", "m16", "m64") for suffix, debug in (("", False), (" debug", True))]
LINE = re.compile(r"(\d+) objects changed \(([^)]*)\); (\d+) identical, (\d+) failed on both sides, (\d+) failed on one side of (\d+)")
MIN_IDENTICAL = 0.5  # of the cells of a leg that must compile and match for it to say anything


def verdict(leg: str, output: str, expected: dict) -> str | None:
    """Why the leg's `objsweep` output fails, or None."""
    found = LINE.search(output)
    if not found:
        return f"{leg}: objsweep printed no result line"
    changed, _, identical, _, one_side, cells = (int(x) if x.isdigit() else x for x in found.groups())
    allowed = expected.get(leg, {}).get("changed", 0)
    if one_side:
        return f"{leg}: {one_side} objects compiled on one side only"
    if changed != allowed:
        return f"{leg}: {changed} objects changed, {allowed} expected ({expected.get(leg, {}).get('why', 'none listed in objsweep_expected.toml')})"
    if changed + identical < cells * MIN_IDENTICAL:
        return f"{leg}: only {changed + identical} of {cells} compiled on both sides; the rest failed and say nothing"
    return None


def control(compiler: str) -> str | None:
    """None if the compiler's O2 and Os objects of one program differ, else why the comparison cannot be trusted."""
    import objsweep

    source = next(iter(sorted(Path(os.environ.get("ROWS_WORK", Path.home() / "scratch/rows")).glob("base/b/fib.c"))), None)
    if source is None:
        return "no fib.c under ROWS_WORK/base/b"
    with tempfile.TemporaryDirectory() as scratch:
        a, why = objsweep.compile_one(compiler, source, "m32", "O2", Path(scratch) / "a.o", "dag")
        b, _ = objsweep.compile_one(compiler, source, "m32", "Os", Path(scratch) / "b.o", "dag")
    if a is None or b is None:
        return f"control did not compile: {why}"
    if a == b:
        return "control: O2 and Os objects are byte-identical, so the comparison sees nothing"
    return seeded(a)


def seeded(good: bytes) -> str | None:
    """None if `good` with one byte flipped is seen as a different object and a leg that changed it fails; else why the leg cannot be trusted."""
    bad = good[:-1] + bytes([good[-1] ^ 1])
    if bad == good:
        return "control: the seeded bad object equals the good one"
    line = "objsweep --isel=dag old a new b: 1 objects changed (m32, O2,Os); 141 identical, 0 failed on both sides, 0 failed on one side of 142"
    return None if verdict("seeded", line, {}) else "control: a leg with a changed object passed"


def main() -> int:
    import llrmbin
    import measure

    os.environ.update(llrmbin.cc_environment())
    head = subprocess.run([sys.executable, str(ROOT / "tools/measure.py"), "dist"], capture_output=True, text=True, check=True).stdout.strip().splitlines()[-1]
    base_sha = measure.base_with_cc("HEAD", measure.base_of("HEAD", os.environ.get("LLRM_MEASURE_REF", "origin/main")))
    if base_sha is None:
        print("objsweep: no ancestor with llrm-cc to compare with")
        return 1
    old, new = str(measure.built(base_sha, "dist") / "llrm-cc"), str(Path(head) / "llrm-cc")
    expected = tomllib.loads((HERE / "objsweep_expected.toml").read_text()).get("leg", {})
    failures = [why] if (why := control(new)) else []
    for leg, targets, debug in LEGS:
        done = subprocess.run(
            [sys.executable, str(HERE / "objsweep.py"), old, new, "--targets", targets, "--levels", "O2,Os", "--isel", "dag", *(["--debug"] if debug else [])],
            capture_output=True, text=True,
        )
        print(f"== {leg}\n{done.stdout}{done.stderr}")
        if why := verdict(leg, done.stdout, expected):
            failures.append(why)
    print("OBJSWEEP " + ("FAIL: " + "; ".join(failures) if failures else "PASS"))
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
