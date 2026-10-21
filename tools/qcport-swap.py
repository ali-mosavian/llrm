#!/usr/bin/env python3
"""Whole-run executed instructions of QCport arms that differ in one module.

    QCPORT=~/scratch/qcport/src QCPORT_INC=~/scratch/qctc/inc QCPORT_BORLAND=~/scratch/qcbcc JWLINK=... \\
        tools/qcport-swap.py d_faces BASE/llrm-c parity=PARITY/llrm-c head=HEAD/llrm-c

Every module but MODULE is built once by BASE at -O2; each arm swaps in MODULE built by its compiler. The BENCH run (`start.qmp -ticks 300`)
executes in the dosrun emulator on virtual time and prints the instructions it executed, the md5 of each arm's EXE and of MODULE's object.
BASE runs twice: the repeat must match the first within 0.001% (the work directory's path moves the count by ~100), and arms with the same EXE
must match exactly, or the instrument is wrong.
Arms that draw a different frame or polygon count did different work: no comparison is made. Per-function counts are not read (the
emulator's profile is unsized for QCport).
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "dosbatch"))


def verdict(arms: list[dict]) -> list[str]:
    """What is wrong with `arms` (dicts of name, exe md5, instructions, frames, polys), empty when the numbers compare."""
    problems = []
    for one in arms:
        for other in arms:
            if one["name"] < other["name"] and one["exe"] == other["exe"] and one["instructions"] != other["instructions"]:
                problems.append(f"{one['name']} and {other['name']} are the same EXE and ran {one['instructions']} and {other['instructions']} instructions")
    first = arms[0]
    for one in arms[1:]:
        if one["name"] == "base-again" and abs(one["instructions"] - first["instructions"]) * 100_000 > first["instructions"]:
            problems.append(f"the repeat of base differs by {one['instructions'] - first['instructions']} instructions (over 0.001%): the count is noise")
    work = {(one["frames"], one["polys"]) for one in arms}
    if len(work) > 1:
        problems.append("the arms drew different work (frames, polys): " + ", ".join(f"{one['name']} {one['frames']}/{one['polys']}" for one in arms))
    return problems


def report(arms: list[dict]) -> str:
    base = arms[0]["instructions"]
    lines = [f"{'arm':<12}{'instructions':>14}{'vs ' + arms[0]['name']:>14}  frames polys  exe md5  obj md5"]
    for one in arms:
        delta = one["instructions"] - base
        lines.append(f"{one['name']:<12}{one['instructions']:>14}{delta:>+14}  {one['frames']} {one['polys']}  {one['exe'][:8]}  {one['obj'][:8]}")
    return "\n".join(lines)


def instructions_of(events: str) -> int:
    ends = [json.loads(line)["instructions"] for line in events.splitlines() if '"ev":"end"' in line]
    if len(ends) != 2:  # the launch's set-up job, then the program
        raise RuntimeError(f"expected the set-up and the program's end events, found {len(ends)}")
    return ends[1]


def md5(path: Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest()


def main() -> int:
    spec = importlib.util.spec_from_file_location("qcport_run", HERE / "qcport-run.py")
    qr = importlib.util.module_from_spec(spec)
    sys.modules["qcport_run"] = qr
    spec.loader.exec_module(qr)
    import dosbatch

    module, base, *named = sys.argv[1:]
    arms = [("base", Path(base)), *((name, Path(path)) for name, path in (one.split("=", 1) for one in named))]
    arms.insert(1, ("base-again", Path(base)))
    qcport, include, borland = qr.required("QCPORT"), qr.required("QCPORT_INC"), qr.required("QCPORT_BORLAND")
    linker = os.environ.get("JWLINK", "jwlink")
    flags = [flag for part in (*qr.MODULES, "qgl") for flag in ("-I", str(qcport / part))] + ["-I", str(include)]
    sources = sorted(source for part in qr.MODULES for source in (qcport / part).glob("*.c"))
    swapped = next(source for source in sources if source.stem == module)
    work = dosbatch.private_work("qcport-swap")
    shared = work / "shared"
    shared.mkdir(parents=True)
    for one in [*borland.glob("*.obj"), *(borland / name for name in qr.DATA)]:
        if one.exists():
            shutil.copy(one, shared / one.name)
    for source in sources:
        if (why := qr.compile_failure(Path(base), flags, source, shared / f"{source.stem}.obj")):
            sys.exit(f"base: {why}")
    stand_in = subprocess.run([base, "-O1", "-fno-tree-loop-distribute-patterns", "-I", str(include), str(HERE / "qcport" / "strlib.c"), "-o", str(shared / "strlib.obj")], capture_output=True, text=True)
    if stand_in.returncode:
        sys.exit(f"strlib: {stand_in.stderr.strip()}")
    results = []
    for name, compiler in arms:
        side = work / name
        shutil.copytree(shared, side)
        if (why := qr.compile_failure(compiler, flags, swapped, side / f"{module}.obj")):
            sys.exit(f"{name}: {why}")
        (side / "QCPORT.LNK").write_text(qr.linked((borland / "QCPORT.LNK").read_text(), side, ["strlib.obj"]))
        subprocess.run([linker, f"@{side / 'QCPORT.LNK'}"], capture_output=True, text=True)
        if not (side / "QCPORT.EXE").exists():
            sys.exit(f"{name}: link failed")
        data = tuple(side / one for one in qr.DATA if (side / one).exists())
        job = dosbatch.Job("J", "exe", side / "QCPORT.EXE", files=data, args=qr.ARGUMENTS, screen=True, budget_ms=120_000)
        done = dosbatch.run([job], work / f"run-{name}", conf=dosbatch.CONF.replace("[mixer]\nnosound=true\n", "[mixer]\nnosound=true\n[sblaster]\nsbtype=sb16\nsbbase=220\nirq=7\ndma=1\nhdma=5\n"))["J"]
        if done.status != "ok":
            sys.exit(f"{name}: run {done.status} {done.detail}")
        seen = dict(re.findall(r"^(frames|polys)\s+(\d+)", (work / f"run-{name}" / "BENCH.TXT").read_text(), re.M))
        results.append({"name": name, "exe": md5(side / "QCPORT.EXE"), "obj": md5(side / f"{module}.obj"), "frames": seen.get("frames"), "polys": seen.get("polys"),
                        "instructions": instructions_of((work / f"run-{name}" / "events.txt").read_text())})
    print(report(results))
    problems = verdict(results)
    print(*problems, sep="\n")
    if not problems:
        dosbatch.discard(work)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
