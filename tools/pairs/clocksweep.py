#!/usr/bin/env python3
"""Clocks of every runnable program at m16 -Os, two builds side by side: the programs that got slower.

    tools/pairs/clocksweep.py measure OUT.json           build and run every program with the dist in $LLRM_BIN
    tools/pairs/clocksweep.py compare BASE.json NEW.json  the programs slower in NEW, with the % and the clocks
    tools/pairs/clocksweep.py sweep BASE_DIST NEW_DIST [--isel dag]   both, one report

A program is each tests/run, examples and bench source (run_tests.discover) built for real mode at -Os with its own header's other flags,
run whole (start-up, runtime and all) in the emulator of `crates/target/llrm-x86-m16/bench/icount.py` and priced by the 486's clocks (vsgcc's
table). Output is checked against NAME.out: a program that prints something else on either side is void, not a result. A program whose count is
near the start-up's is `tiny`: its source may have folded to a constant, and a difference of a few clocks says nothing.
"""

from __future__ import annotations

import os
import re
import sys
import json
import math
import shutil
import argparse
import tempfile
import subprocess
from pathlib import Path
from concurrent.futures import ProcessPoolExecutor

ROOT = Path(__file__).resolve().parents[2]
for path in ("tools/dosbatch", "tools", "crates/target/llrm-x86-m16/bench"):
    sys.path.insert(0, str(ROOT / path))

LIMIT = 250_000_000  # instructions per program: the emulator steps about 10^6 a second; grep.bas runs 186 M
TINY = 2.0  # a program under this many times the least count of its language is within the start-up
MARGIN = 0.0  # a program is slower by any positive amount; the report sorts by it


def with_level(flags: list[str]) -> list[str]:
    """`flags` at -Os: the program's own other flags (`-march=`, `-fno-...`), its level replaced."""
    return ["-Os" if one.startswith("-O") else one for one in flags] if any(one.startswith("-O") for one in flags) else [*flags, "-Os"]


def programs():
    """Every real-mode program once, flags at -Os."""
    import run_tests
    import dosbatch
    import dataclasses

    seen, out = set(), []
    for one in run_tests.discover([]):
        if dosbatch.target_of(one.flags, dosbatch.REAL_MODE) != dosbatch.REAL_MODE or run_tests.unavailable(one):
            continue
        one = dataclasses.replace(one, flags=with_level(one.flags), label=re.sub(r"-O\w+", "-Os", one.label))
        key = (str(one.source), one.dialect, tuple(one.flags))
        if key not in seen:
            seen.add(key)
            out.append(one)
    return out


def run_one(job: tuple) -> dict:
    """(clocks, instructions, output) of one linked program run whole; `error` where it did not run to its exit."""
    import icount

    exe, files, stdin, limit = job
    with tempfile.TemporaryDirectory() as directory:
        for name, source in files:
            shutil.copy(source, Path(directory) / name)
        result = icount.run(Path(exe), None, Path(directory), limit=limit, whole=True, stdin=stdin)
    if result.error or result.region is None:
        return {"error": result.error or "the program never exited (limit)"}
    return {"clocks": result.region.clocks, "instructions": result.region.instructions, "output": result.output.decode("latin1")}


def measure(out: Path, select: list[str], limit: int, dosbox: bool = False) -> None:
    import run_tests
    import dosbatch
    import dataclasses
    from concurrent.futures import ThreadPoolExecutor

    todo = [one for one in programs() if not select or any(word in one.name for word in select)]
    work = dosbatch.private_work("clocksweep")
    objs = work.with_name(work.name + "-obj")
    objs.mkdir(parents=True, exist_ok=True)
    stems = {one.name: f"T{at:03d}" for at, one in enumerate(todo)}
    with ThreadPoolExecutor() as pool:
        built = dict(zip((one.name for one in todo), pool.map(lambda one: run_tests.build(one, objs, stems[one.name]), todo)))
    exes, rows = {}, {}
    for one in todo:
        job = built[one.name]
        if isinstance(job, str):
            rows[one.name] = {"error": job[:200]}
        elif job.kind == "exe":
            exes[one.name] = (job.path, job)
    for dialect, tools in run_tests.TOOLS.items():
        linking = {one.name: dataclasses.replace(built[one.name], budget_ms=500, map=False) for one in todo if one.dialect == dialect and not isinstance(built[one.name], str) and built[one.name].kind == "obj"}
        if not linking:
            continue
        ran = dosbatch.run(list(linking.values()), work / f"link-{dialect}", tools=tools)
        for name, job in linking.items():
            exe = work / f"link-{dialect}" / f"{job.stem.upper()}.EXE"
            if ran[job.stem].status == "not built" or not exe.exists():
                rows[name] = {"error": "link: " + ran[job.stem].detail[:200]}
            else:
                exes[name] = (exe, job)
    jobs, names = [], []
    for one in todo:
        if one.name not in exes:
            continue
        exe, job = exes[one.name]
        files = [(path.name.upper(), path) for path in run_tests.data_files(one)]
        stdin = (one.source.parent / one.stdin).read_bytes() if one.stdin else None
        jobs.append((str(exe), files, stdin, limit))
        names.append(one)
    with ProcessPoolExecutor(max_workers=max(1, (os.cpu_count() or 2) // 2)) as pool:
        for one, got in zip(names, pool.map(run_one, jobs)):
            if "error" not in got:
                want = run_tests.lines(one.out.read_text())
                seen = run_tests.masked(run_tests.lines(got.pop("output")), one.mask)
                if seen != want:
                    got["error"] = "prints another output: " + run_tests.first_difference(want, seen)[:120]
            rows[one.name] = got
    if dosbox:
        # The whole program's RDTSC cycles under DOSBox's fixed-cycle clock (tools/bench/bench.py time_programs): what the emulator's counts are checked against.
        sys.path.insert(0, str(ROOT / "tools" / "bench"))
        import bench

        timed = bench.time_programs({name: exes[name][0] for name in exes if "clocks" in rows.get(name, {})}, work / "timing")
        for name, got in timed.items():
            rows[name]["dosbox_cycles"] = got.get("cycles", got.get("error"))
    language = {one.name: one.source.suffix for one in todo}
    for name, row in rows.items():
        row["language"] = language.get(name, "")
    out.write_text(json.dumps({"isel": os.environ.get("LLRM_ISEL", ""), "bin": os.environ.get("LLRM_BIN", ""), "rows": rows}, indent=1))
    dosbatch.discard(work)
    print(f"{out}: {sum('clocks' in row for row in rows.values())} of {len(rows)} programs measured")


LOCAL_PATH = re.compile(r"(?:/home|/Users|/tmp|~)/\S*")


def scrub(text: str) -> str:
    """Machine paths cut to their last two components: a report is committed, and a path names its machine."""
    return LOCAL_PATH.sub(lambda found: "/".join(found.group().rstrip("/").split("/")[-2:]), text)


def compare(base: dict, new: dict) -> list[str]:
    """The report: arms, void programs, then the programs slower in `new`, worst first."""
    before, after = base["rows"], new["rows"]
    both = sorted(set(before) & set(after))
    void = [name for name in both if "clocks" not in before[name] or "clocks" not in after[name]]
    good = [name for name in both if name not in void]
    floor: dict[str, int] = {}
    for name in good:
        language = before[name]["language"]
        floor[language] = min(floor.get(language, 1 << 60), before[name]["instructions"])
    lines = [f"arms: base {base['bin'] or '?'} isel={base['isel'] or 'default'}; new {new['bin'] or '?'} isel={new['isel'] or 'default'}",
             f"{len(good)} programs measured on both, {len(void)} void (did not build, run or print the expected output on one side)"]
    slower = sorted(((after[name]["clocks"] - before[name]["clocks"]) / max(1, before[name]["clocks"]), name) for name in good if after[name]["clocks"] > before[name]["clocks"] * (1 + MARGIN))
    tiny = {name for name in good if before[name]["instructions"] < TINY * floor[before[name]["language"]]}
    ratios = [after[name]["clocks"] / max(1, before[name]["clocks"]) for name in good]
    lines.append(f"{len(slower)} slower, {sum(after[n]['clocks'] < before[n]['clocks'] for n in good)} faster, {len(tiny)} tiny (near the start-up); geomean new/base = {math.exp(sum(map(math.log, ratios)) / len(ratios)):.4f}" if ratios else "nothing measured on both")
    for ratio, name in reversed(slower):
        lines.append(f"  {ratio * 100:+7.2f}%  {after[name]['clocks'] - before[name]['clocks']:+10d} clocks  ({before[name]['clocks']} -> {after[name]['clocks']}){'  tiny' if name in tiny else ''}  {name}")
    for name in void:
        why = before[name].get("error") or after[name].get("error") or "?"
        lines.append(f"  void  {name}: {'base' if 'error' in before[name] else 'new'}: {why[:100]}")
    return [scrub(line) for line in lines]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    one = sub.add_parser("measure")
    one.add_argument("out", type=Path)
    one.add_argument("select", nargs="*")
    one.add_argument("--limit", type=int, default=LIMIT)
    one.add_argument("--dosbox", action="store_true", help="also time each program under DOSBox (RDTSC cycles)")
    two = sub.add_parser("compare")
    two.add_argument("base", type=Path)
    two.add_argument("new", type=Path)
    three = sub.add_parser("sweep")
    three.add_argument("base", type=Path)
    three.add_argument("new", type=Path)
    three.add_argument("--isel", default="", help="LLRM_ISEL for the new arm")
    three.add_argument("--work", type=Path, default=Path(tempfile.gettempdir()))
    three.add_argument("select", nargs="*")
    args = parser.parse_args()
    if args.command == "measure":
        measure(args.out, args.select, args.limit, args.dosbox)
    elif args.command == "compare":
        print("\n".join(compare(json.loads(args.base.read_text()), json.loads(args.new.read_text()))))
    else:
        outs = []
        for arm, dist, isel in (("base", args.base, ""), ("new", args.new, args.isel)):
            out = args.work / f"clocks-{arm}.json"
            # LLRM_ISEL_REAL: run_tests keeps a real-mode C program on the old selector otherwise
            env = {**os.environ, "LLRM_BIN": str(dist), **({"LLRM_ISEL": isel, "LLRM_ISEL_REAL": "1"} if isel else {})}
            if not isel:
                env.pop("LLRM_ISEL", None)
                env.pop("LLRM_ISEL_REAL", None)
            subprocess.run([sys.executable, __file__, "measure", str(out), *args.select], env=env, check=True)
            outs.append(out)
        print("\n".join(compare(json.loads(outs[0].read_text()), json.loads(outs[1].read_text()))))
    return 0


if __name__ == "__main__":
    sys.exit(main())
