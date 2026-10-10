"""Graphics primitives and text in each screen mode, on BCOM45 (BC + LINK), the llrm runtime on m16 and on m32, all in
DOSBox on virtual time: a segment's time is the emulated milliseconds its TIMER reads, `RATE` guest instructions each.

    python tools/dosbatch/gfxbench/run.py [--modes 0 1 2 ...] [--work DIR]
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbruntime  # noqa: E402

MODES = (0, 1, 2, 7, 8, 9, 12, 13)
RATE = 2000
CONF = qb32.virtual_conf(RATE)
BUDGET_MS = 3_000_000
COLUMNS = ("BCOM45 m16", "llrm+BCOM45", "llrm m16", "llrm m32")


def source(mode: int, work: Path) -> Path:
    path = work / f"g{mode}.bas"
    text = (HERE / "gfxbench.bas").read_text().replace("@MODE@", str(mode)).replace("@RESULT@", f"G{mode}.RES")
    path.write_bytes(text.replace("\n", "\r\n").encode())
    return path


def parse(text: str) -> dict[str, int]:
    found = {}
    for line in text.splitlines():
        if line.split():
            *label, value = line.split()
            found[" ".join(label)] = int(value)
    return found


def run(modes: list[int], work: Path) -> dict[int, dict[str, dict[str, int]]]:
    work.mkdir(parents=True, exist_ok=True)
    sources = {mode: source(mode, work) for mode in modes}
    archive, _ = qbruntime.build(work / "archive16")
    runtime32 = qb32.build(work / "rt32")
    sides: dict[str, tuple[Path, list[dosbatch.Job]]] = {}
    jobs = []
    for mode, path in sources.items():
        jobs.append(dosbatch.Job(f"B{mode:02d}", "bas", path, screen=True, budget_ms=BUDGET_MS, switches="/O /FPi"))
    sides["BCOM45 m16"] = (work / "run_bc", jobs)
    jobs = []
    for mode, path in sources.items():
        obj = work / f"g{mode}.qb45.obj"
        if reason := qbruntime.compile_basic(path, obj, "qb45"):
            raise SystemExit(f"qb45 g{mode}: {reason}")
        jobs.append(dosbatch.Job(f"Q{mode:02d}", "obj", obj, screen=True, budget_ms=BUDGET_MS))
    sides["llrm+BCOM45"] = (work / "run_qb45", jobs)
    jobs = []
    for mode, path in sources.items():
        obj = work / f"g{mode}.m16.obj"
        if reason := qbruntime.compile_basic(path, obj, "llrm"):
            raise SystemExit(f"m16 g{mode}: {reason}")
        jobs.append(dosbatch.Job(f"L{mode:02d}", "obj", obj, runtime="llrmqb", runtime_file=archive, screen=True, budget_ms=BUDGET_MS))
    sides["llrm m16"] = (work / "run_m16", jobs)
    jobs = []
    for mode, path in sources.items():
        obj, exe = work / f"g{mode}.m32.obj", work / f"g{mode}.m32.exe"
        if reason := qb32.compile_basic(path, obj):
            raise SystemExit(f"m32 g{mode}: {reason}")
        loaders = qb32.link(obj, runtime32, exe, work)
        jobs.append(dosbatch.Job(f"M{mode:02d}", "exe", exe, files=loaders, screen=True, budget_ms=BUDGET_MS))
    sides["llrm m32"] = (work / "run_m32", jobs)

    found: dict[int, dict[str, dict[str, int]]] = {mode: {} for mode in modes}
    for side, (where, side_jobs) in sides.items():
        results = dosbatch.run(side_jobs, where, conf=CONF, budget_ms=BUDGET_MS)
        for mode, job in zip(modes, side_jobs):
            if results[job.stem].status != "ok":
                print(f"{side} SCREEN {mode}: {results[job.stem].status} {results[job.stem].detail[-200:]}", file=sys.stderr)
            for label, ms in parse(dosbatch.read_dos(where, f"G{mode}.RES")).items():
                found[mode].setdefault(label, {})[side] = ms
    return found


def table(found: dict[int, dict[str, dict[str, int]]]) -> str:
    head = f"{'mode':>4} {'segment':<13}" + "".join(f"{c:>12}" for c in COLUMNS) + f"{'q45/BC':>9}{'m16/BC':>9}{'m32/BC':>9}"
    lines = [f"emulated ms ({RATE} guest instructions each; a TIMER tick is 55 ms)", head]
    for mode, rows in found.items():
        for label, cells in rows.items():
            values = [cells.get(c) for c in COLUMNS]
            ratio = lambda value: f"{value / values[0]:>9.2f}" if value and values[0] else f"{'-':>9}"  # noqa: E731
            lines.append(f"{mode:>4} {label:<13}" + "".join(f"{('-' if v is None else v):>12}" for v in values) + ratio(values[1]) + ratio(values[2]) + ratio(values[3]))
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--modes", nargs="*", type=int, default=list(MODES))
    parser.add_argument("--work", type=Path, default=None)
    args = parser.parse_args()
    print(table(run(args.modes, args.work or Path(tempfile.mkdtemp(prefix="gfxbench-")))))
    return 0


if __name__ == "__main__":
    sys.exit(main())
