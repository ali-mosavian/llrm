#!/usr/bin/env python3
"""What it costs llrm-c to compile: user-space instructions per file at -O1, -O2 and -Os, against a stored baseline.

    tools/compile-cost.py            compare with tools/gate/compile-baseline.json; exit 1 on a rise past the limits
    tools/compile-cost.py --refresh  rewrite the baseline entries that moved (a drop fails too: the baseline is main)
    tools/compile-cost.py --update   measure and store every entry

Files: the 66 vsgcc programs (bench/ and the x_ kernels, wrapped as vsgcc compiles them) and, when QCPORT and QCPORT_INC
are set, QCport's modules. Instructions come from `perf stat -e instructions:u`, one process per compile: a count of
work done, not of time taken, so host load does not move it. Exit 77 if perf cannot count (no permission, a VM with no
counter), never a pass on a zero.

The limits (GEOMEAN_LIMIT, WORST_LIMIT) are set from `--noise`: the same build measured twice, files' counts compared.
A rise is fixed or the baseline refreshed in the same commit, so the cost of a change shows in its diff; so is a drop,
or the next change would give it back unseen.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
VSGCC = next((ROOT / "crates/target").glob("*/vsgcc"))  # the target the comparison is for owns its programs and flags
BASELINE = HERE / "gate" / "compile-baseline.json"
LEVELS = ("-O1", "-O2", "-Os")
SCRATCH = os.environ.get("CARGO_TARGET_DIR")  # never /tmp
MODULES = ("host", "render", "model", "game", "sound", "ui")
REFRESH = "refresh the baseline in this PR (python3 tools/compile-cost.py --refresh)"
GEOMEAN_LIMIT = 1.003  # same build twice (--noise, 393 files, 2026-10-08): geomean within 0.0001, worst file 0.0047
WORST_LIMIT = 1.02
MOVED = 0.005  # --refresh rewrites a file that differs from its baseline by more than this: above the worst noise (0.0047)

sys.path.insert(0, str(HERE))
sys.path.insert(0, str(VSGCC))
import llrmbin  # noqa: E402
import programs  # noqa: E402
import wrap  # noqa: E402


class NoCounter(Exception):
    pass


def instructions(command: list[str]) -> int:
    """User-space instructions `command` retires, children included. The compiler must succeed."""
    with tempfile.NamedTemporaryFile("r", dir=SCRATCH) as out:
        env = {k: v for k, v in os.environ.items() if not k.startswith("LLRM_") or k == "LLRM_BIN"}  # LLRM_VERIFY adds checking work
        done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", "-o", out.name, "--", *command], capture_output=True, text=True, env=env)
        counts = [line.split(",") for line in out.read().splitlines() if "instructions" in line]
    if done.returncode:
        raise SystemExit(f"{' '.join(command[-3:])}: exit {done.returncode}\n{done.stderr[-400:]}")
    if not counts or not counts[0][0].isdigit() or int(counts[0][0]) == 0:
        raise NoCounter(f"perf counted {counts or 'nothing'}")
    return int(counts[0][0])


def files(work: Path) -> dict[str, tuple[Path, list[str]]]:
    """Name -> (source, extra flags): the 66 programs, wrapped as vsgcc/build.sh does, and QCport when it is available."""
    found: dict[str, tuple[Path, list[str]]] = {}
    sources = programs.sources()
    for name, source in sources.items():
        wrapped = work / f"{name}.c"
        wrapped.write_text(wrap.wrapped(name, source.read_text()))
        found[name] = (wrapped, programs.LLRM_FLAGS)
    if (qcport := os.environ.get("QCPORT")) and (headers := os.environ.get("QCPORT_INC")):
        base = Path(qcport).expanduser()
        include = [flag for d in (*MODULES, "qgl") for flag in ("-I", str(base / d))] + ["-I", str(Path(headers).expanduser())]
        for d in MODULES:
            for source in sorted((base / d).glob("*.c")):
                found[f"qcport/{source.stem}"] = (source, include)
    return found


def measure(compiler: Path, jobs: int) -> dict[str, int]:
    """Instructions per `file level`."""
    with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
        work = Path(tmp)
        todo = [(f"{name} {level}", [str(compiler), level, *flags, str(source), "-o", os.devnull]) for name, (source, flags) in files(work).items() for level in LEVELS]
        with ThreadPoolExecutor(jobs) as pool:
            return dict(zip((k for k, _ in todo), pool.map(instructions, (c for _, c in todo))))


def geomean(ratios: list[float]) -> float:
    return math.exp(sum(math.log(r) for r in ratios) / len(ratios))


def compare(baseline: dict[str, int], now: dict[str, int], geomean_limit: float = GEOMEAN_LIMIT, worst_limit: float = WORST_LIMIT) -> tuple[list[str], list[str]]:
    """(report lines, failures). Per level and for QCport / the programs: the geomean of now/baseline, and the worst file.
    A file in only one of the two is a failure: a baseline of other files says nothing about these."""
    lines, bad = [], []
    if not any(k.startswith("qcport/") for k in now):
        lines.append("qcport: not measured (QCPORT and QCPORT_INC are not set)")
        baseline = {k: v for k, v in baseline.items() if not k.startswith("qcport/")}
    for only in sorted(baseline.keys() ^ now.keys()):
        bad.append(f"{only}: in the {'baseline' if only in baseline else 'measurement'} only")
    groups = {"qcport": lambda k: k.startswith("qcport/"), "programs": lambda k: not k.startswith("qcport/")}
    for group, member in groups.items():
        for level in LEVELS:
            ratios = {k: now[k] / baseline[k] for k in baseline.keys() & now.keys() if member(k) and k.endswith(" " + level)}
            if not ratios:
                continue
            g, worst = geomean(list(ratios.values())), max(ratios, key=ratios.get)
            lines.append(f"{group} {level}: geomean {g:.4f}, worst {ratios[worst]:.4f} ({worst.rsplit(' ', 1)[0]}), {len(ratios)} files")
            best = min(ratios, key=ratios.get)
            if g > geomean_limit:
                bad.append(f"{group} {level}: geomean {g:.4f} > {geomean_limit}")
            if ratios[worst] > worst_limit:
                bad.append(f"{group} {level}: {worst} {ratios[worst]:.4f} > {worst_limit}")
            if g < 1 / geomean_limit:
                bad.append(f"{group} {level}: geomean {g:.4f} faster: {REFRESH}")
            if ratios[best] < 1 / worst_limit:
                bad.append(f"{group} {level}: {best} {ratios[best]:.4f} faster: {REFRESH}")
    return lines, bad


def refreshed(baseline: dict[str, int], now: dict[str, int]) -> dict[str, int]:
    """The baseline with the entries that moved past MOVED rewritten, new files added. A file not measured this time (QCport
    without QCPORT) keeps its entry; one measured and gone from the tree loses it."""
    measured = {"qcport" if k.startswith("qcport/") else "programs" for k in now}
    kept = {k: v for k, v in baseline.items() if ("qcport" if k.startswith("qcport/") else "programs") not in measured or k in now}
    new = dict(sorted((k, now[k] if k in now and (k not in kept or abs(now[k] / kept[k] - 1) > MOVED) else kept[k]) for k in kept.keys() | now.keys()))
    # Many files each moved less than MOVED can still add up past the geomean limit: then every measured entry moves.
    return new if not compare(new, now)[1] else dict(sorted({**kept, **now}.items()))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--refresh", action="store_true", help="rewrite the baseline entries that moved")
    parser.add_argument("--update", action="store_true", help="store the measurement as the baseline")
    parser.add_argument("--noise", action="store_true", help="measure twice, print the spread (not a comparison)")
    parser.add_argument("--baseline", type=Path, default=BASELINE)
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "4")))
    parser.add_argument("compiler", nargs="?", type=Path)
    args = parser.parse_args()
    args.compiler = args.compiler or llrmbin.bin_dir() / "llrm-c"
    try:
        now = measure(args.compiler, args.jobs)
        if args.noise:
            again = measure(args.compiler, args.jobs)
            lines, _ = compare(now, again, 0, 0)
            spread = sorted(abs(again[k] / v - 1) for k, v in now.items())
            print("\n".join(lines), f"files {len(now)}, spread median {spread[len(spread) // 2]:.6f}, p99 {spread[int(len(spread) * .99)]:.6f}, max {spread[-1]:.6f}", sep="\n")
            return 0
    except NoCounter as why:
        print(f"SKIPPED: instruction counter unavailable ({why})")
        return 77
    if args.refresh:
        old = json.loads(args.baseline.read_text())
        new = refreshed(old, now)
        args.baseline.write_text(json.dumps(new, indent=0) + "\n")
        print(f"baseline: {sum(new.get(k) != v for k, v in old.items())} entries moved, {len(new.keys() - old.keys())} added, {len(old.keys() - new.keys())} gone -> {args.baseline}")
        return 0
    if args.update:
        args.baseline.write_text(json.dumps(dict(sorted(now.items())), indent=0) + "\n")
        print(f"baseline: {len(now)} measurements -> {args.baseline}")
        return 0
    lines, bad = compare(json.loads(args.baseline.read_text()), now)
    print("\n".join(lines))
    if bad:
        print("COMPILE COST RISE:", *bad, sep="\n  ")
        return 1
    print(f"compile cost within {GEOMEAN_LIMIT} geomean / {WORST_LIMIT} worst of {args.baseline.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
