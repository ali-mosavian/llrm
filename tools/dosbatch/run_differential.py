"""
tests/differential/qb-bc: each program compiled by BC 4.5 and by llrm-qb, both
run in one DOSBox launch. NAME.out is what both print; with `' diverges:` in the
header BC prints something else (the reason follows) and only llrm must match.

    python3 tools/dosbatch/run_differential.py [NAME]...

Header: `' bc: SWITCHES` (BC's), `' flags: FLAGS` (llrm-qb's), `' diverges: WHY`.
"""

from __future__ import annotations

import sys
import argparse
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
llrmbin = dosbatch.llrmbin
import run_tests  # noqa: E402
from dosbatch import ROOT, Job  # noqa: E402
from run_tests import Program, first_difference, lines  # noqa: E402

CASES = ROOT / "tests" / "differential" / "qb-bc"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("select", nargs="*")
    parser.add_argument("--work", type=Path, help="where to build and run; one private to this run by default")
    args = parser.parse_args()
    sources = [one for one in sorted(CASES.glob("*.bas")) if one.with_suffix(".out").exists() and (not args.select or one.stem in args.select)]
    if not sources:
        print("no programs selected")
        return 1
    settings = {one.stem: run_tests.header(one) for one in sources}
    own = args.work is None
    work = dosbatch.private_work("tests-differential") if own else args.work
    objs = work.with_name(work.name + "-obj")
    objs.mkdir(parents=True, exist_ok=True)
    stems = {one.stem: f"T{at:03d}" for at, one in enumerate(sources)}
    programs = [Program(one, settings[one.stem].get("flags", "").split(), None) for one in sources]
    with ThreadPoolExecutor() as pool:
        failed = dict(zip((one.stem for one in sources), pool.map(lambda p: run_tests.compile_one(p, objs / f"{stems[p.source.stem]}.obj"), programs)))
    jobs = []
    for one in sources:
        stem = stems[one.stem]
        jobs.append(Job(f"B{stem[1:]}", "bas", one, switches=settings[one.stem].get("bc", "/O")))
        if not failed[one.stem]:
            jobs.append(Job(stem, "obj", objs / f"{stem}.obj"))
    ran = dosbatch.run(jobs, work)
    bad = 0
    for one in sources:
        golden = lines(one.with_suffix(".out").read_text())
        stem = stems[one.stem]
        diverges = "diverges" in settings[one.stem]
        problems = []
        bc = ran[f"B{stem[1:]}"]
        if bc.status != "ok":
            problems.append(f"BC {bc.status}: {bc.detail}")
        elif diverges and not first_difference(golden, lines(bc.text)):
            problems.append("BC now agrees: this is no divergence")
        elif not diverges and (difference := first_difference(golden, lines(bc.text))):
            problems.append(f"BC's output is not the .out: {difference}")
        if failed[one.stem]:
            problems.append(failed[one.stem])
        elif ran[stem].status != "ok":
            problems.append(f"llrm {ran[stem].status}: {ran[stem].detail}")
        elif difference := first_difference(golden, lines(ran[stem].text)):
            problems.append(f"llrm differs from the .out: {difference}")
        if problems:
            bad += 1
            print(f"FAIL  {one.stem}: " + "; ".join(problems))
    print(f"{len(sources)} programs: {len(sources) - bad} pass, {bad} fail")
    if own and not bad:
        dosbatch.discard(work)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
