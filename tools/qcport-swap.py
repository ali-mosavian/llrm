#!/usr/bin/env python3
"""Whole-run executed instructions of QCport arms that differ in one module.

    QCPORT=~/scratch/qcport/src QCPORT_INC=~/scratch/qctc/inc QCPORT_BORLAND=~/scratch/qcbcc JWLINK=... \\
        tools/qcport-swap.py d_faces BASE/COMPILER parity=PARITY/COMPILER head=HEAD/COMPILER

Every module but MODULE is built once by BASE at -O2; each arm swaps in MODULE built by its compiler. The BENCH run (`start.qmp -ticks 300`)
executes in the dosrun emulator on virtual time and prints the instructions it executed, the md5 of each arm's EXE and of MODULE's object.
BASE runs 1 + QCPORT_SWAP_REPEATS (3) times: every repeat must match the first within 0.001% (the work directory's path moves the count by ~100), and arms with the same EXE
must match exactly, or the instrument is wrong.
Arms that draw a different frame or polygon count did different work: no comparison is made. Per-function counts (the emulator's profile, unsized for QCport: trust a total, not a split) are flagged only above the
largest move of a function between BASE and its repeat, whose objects are the same: EMS paging order moves millions in balanced pairs.
"""
from __future__ import annotations

import hashlib
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
        if one["name"].startswith("base-again") and abs(one["instructions"] - first["instructions"]) * 100_000 > first["instructions"]:
            problems.append(f"{one['name']}, the repeat of base, differs by {one['instructions'] - first['instructions']} instructions (over 0.001%): the count is noise")
    work = {(one["frames"], one["polys"]) for one in arms}
    if len(work) > 1:
        problems.append("the arms drew different work (frames, polys): " + ", ".join(f"{one['name']} {one['frames']}/{one['polys']}" for one in arms))
    return problems


def functions_of(events: str) -> dict[str, int]:
    """The emulator's profile: instructions executed in each function itself, by name."""
    out: dict[str, int] = {}
    for line in events.splitlines():
        if '"ev":"profile"' in line:
            out.update({one["name"]: one["self"] for one in json.loads(line)["functions"]})
    return out


def deltas(
    base: dict[str, int],
    other: dict[str, int],
) -> dict[str, int]:
    return {name: other.get(name, 0) - count for name, count in base.items() if other.get(name, 0) != count}


def noise(
    base: dict[str, int],
    again: dict[str, int],
) -> tuple[int, int]:
    """What a run of identical objects moves: the sum of the rises and of the falls of its functions. EMS paging order moves
    millions between functions in balanced pairs (QGLGEMMAP +7.4M, QGLGEMFREE -7.4M) with the objects the same."""
    moved = deltas(base, again).values()
    return sum(one for one in moved if one > 0), -sum(one for one in moved if one < 0)


def paired(changes: dict[str, int]) -> set[str]:
    """The functions whose change another function undoes to within a tenth: a rise and a fall of the same size are the EMS paging
    order moving work between functions (QGLGEMMAP +7.4M with QGLGEMFREE -7.4M), not a cost of the module."""
    out: set[str] = set()
    for name, change in changes.items():
        for other, back in changes.items():
            if other != name and change * back < 0 and abs(change + back) * 10 <= max(abs(change), abs(back)):
                out.update((name, other))
    return out


def flagged(
    base: dict[str, int],
    other: dict[str, int],
    floor: int,
) -> dict[str, int]:
    """The functions whose change is more than the noise of identical objects."""
    return {name: change for name, change in deltas(base, other).items() if abs(change) > floor}


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
    sys.path.insert(0, str(HERE))
    import qcport_run as qr  # noqa: PLC0415
    import dosbatch

    module, base, *named = sys.argv[1:]
    arms = [("base", Path(base)), *((name, Path(path)) for name, path in (one.split("=", 1) for one in named))]
    repeats = int(os.environ.get("QCPORT_SWAP_REPEATS", "3"))
    arms[1:1] = [(f"base-again{n}", Path(base)) for n in range(1, repeats + 1)]
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
        sidecar = work / f"map-{name}"
        sidecar.mkdir()
        shutil.copy(side / "QCPORT.MAP", sidecar / "J.MAP")
        job = dosbatch.Job("J", "exe", side / "QCPORT.EXE", files=(*data, sidecar / "J.MAP"), args=qr.ARGUMENTS, screen=True, budget_ms=120_000, profile=True)
        done = dosbatch.run([job], work / f"run-{name}", conf=dosbatch.CONF.replace("[mixer]\nnosound=true\n", "[mixer]\nnosound=true\n[sblaster]\nsbtype=sb16\nsbbase=220\nirq=7\ndma=1\nhdma=5\n"))["J"]
        if done.status != "ok":
            sys.exit(f"{name}: run {done.status} {done.detail}")
        seen = dict(re.findall(r"^(frames|polys)\s+(\d+)", (work / f"run-{name}" / "BENCH.TXT").read_text(), re.M))
        results.append({"name": name, "exe": md5(side / "QCPORT.EXE"), "obj": md5(side / f"{module}.obj"), "frames": seen.get("frames"), "polys": seen.get("polys"),
                        "instructions": instructions_of((work / f"run-{name}" / "events.txt").read_text()),
                        "functions": functions_of((work / f"run-{name}" / "events.txt").read_text())})
    print(report(results))
    repeated = [one["functions"] for one in results if one["name"].startswith("base-again")]
    floor = max((abs(change) for again in repeated for change in deltas(results[0]["functions"], again).values()), default=0)
    rises = max((noise(results[0]["functions"], again)[0] for again in repeated), default=0)
    falls = max((noise(results[0]["functions"], again)[1] for again in repeated), default=0)
    print(f"noise of identical objects, {len(repeated)} repeats of base: functions up {rises}, down {falls} (worst repeat); a function is flagged above {floor}")
    for one in results[1 + len(repeated):]:
        changes = flagged(results[0]["functions"], one["functions"], floor)
        for name, change in sorted(changes.items(), key=lambda pair: -abs(pair[1]))[:10]:
            print(f"  {one['name']}: {name} {change:+}{'  paired: noise' if name in paired(changes) else ''}")
    problems = verdict(results)
    print(*problems, sep="\n")
    if not problems:
        dosbatch.discard(work)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
