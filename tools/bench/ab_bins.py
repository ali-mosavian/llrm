"""Two compiler builds on every m16 bench program at -O1, -O2 and -Os: bytes and executed clocks per cell, base against head.

    python3 tools/bench/ab_bins.py BASE_BIN HEAD_BIN [--levels -O1,-O2,-Os] [--only fib,matmul] [--jobs 2]
        [--base-env NAME=VALUE]... [--head-env NAME=VALUE]...

BASE_BIN and HEAD_BIN are dist directories (llrm-cc, jwasm, jwlink); the two may be the same directory with different --base-env/--head-env. Run it at every pin bump and after any tuning-table change: a knob
is judged on the whole corpus, not on the cell it was found on. A program whose kernel the compiler folded into `main` is counted over the
whole program (runtime start-up is the same on both sides). Verdicts follow bench.py's rules: at -O2 head is worse when 1000 * dclk +
rate * dB > 0 (rate: bench.rate_for, 500 on m16: one clock is two bytes), at -Os on bytes alone, at -O1 on either axis (one better and the
other worse is a "trade"). A cell whose program output differs between the builds is "output differs", never a verdict.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from multiprocessing import Pool
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LEVELS = ("-O1", "-O2", "-Os")


def verdict(level: str, rate: int, base: tuple[int, int], head: tuple[int, int]) -> str:
    """`head` against `base`, both (bytes, clocks): better, same, worse or trade."""
    dbytes, dclk = head[0] - base[0], head[1] - base[1]
    if level == "-Os":
        score = dbytes
    elif level == "-O2":
        score = 1000 * dclk + rate * dbytes
    else:
        up, down = dbytes > 0 or dclk > 0, dbytes < 0 or dclk < 0
        return "trade" if up and down else "worse" if up else "better" if down else "same"
    return "worse" if score > 0 else "better" if score < 0 else "same"


def programs(bench: Path, only: set[str] | None = None) -> list[tuple[str, str]]:
    """(source, kernel symbol) of each `bench/NAME/NAME.c`."""
    found = []
    for folder in sorted(p for p in bench.iterdir() if p.is_dir()):
        if (folder / f"{folder.name}.c").exists() and (only is None or folder.name in only):
            found.append((f"bench/{folder.name}/{folder.name}.c", f"bench_{folder.name}"))
    return found


def measure(job: tuple[str, str, str, str, tuple]) -> tuple[int, int, str] | str:
    """(code bytes, clocks, program output) of one build of one cell; a string is why it could not be measured. One process per
    measurement: `dosbatch` reads LLRM_BIN once, at import, and links the runtime from that directory, so two builds in one process
    would link one build's runtime into both."""
    bindir, source, region, level, extra = job
    done = subprocess.run([sys.executable, __file__, "--measure", json.dumps(job)], capture_output=True, text=True, env={**os.environ, "LLRM_BIN": bindir})
    try:
        said = json.loads(done.stdout.strip().splitlines()[-1])
    except (IndexError, ValueError):
        return "measure: " + (done.stderr or done.stdout)[-120:]
    return said if isinstance(said, str) else tuple(said)


def measure_here(job: tuple[str, str, str, str, tuple]) -> tuple[int, int, str] | str:
    bindir, source, region, level, extra = job
    sys.path[:0] = [str(ROOT / "tools"), str(ROOT / "tools/bench"), str(ROOT / "crates/target/llrm-x86-m16/bench"), str(ROOT / "tools/dosbatch")]
    import dosbatch, icount, maps, sizes  # noqa: E401

    work = Path(tempfile.mkdtemp(prefix="ab."))
    obj, exe, lst = work / "a.obj", work / "a.exe", work / "a.map"
    env = {**os.environ, "LLRM_CC_TARGET": "x86_16-wat", **dict(extra)}
    try:
        done = subprocess.run([f"{bindir}/llrm-cc", "-march=i386", level, "-I", "bench", "-I", "tests/run/c", "-c", source, "-o", str(obj)], capture_output=True, text=True, env=env, cwd=ROOT)
        if done.returncode:
            return "compile: " + done.stderr[-120:]
        dosbatch.link_c(obj, exe, work, lst)
        if "jpeg" in source:
            shutil.copy(ROOT / "bench/jpeg_single/photo.jpg", work / "PHOTO.JPG")
        run = icount.run(exe, maps.locate(exe, lst, region), work)
        if run.region is None and run.error == icount.NEVER_ENTERED:
            run = icount.run(exe, None, work, whole=True)
        if run.region is None:
            return run.error or "no region"
        return sizes.code_size(obj).code, run.region.clocks, run.output.decode("latin1").strip()
    except Exception as err:
        return repr(err)[:120]
    finally:
        shutil.rmtree(work, ignore_errors=True)


def compare(base_bin: str, head_bin: str, cells, levels, rate: int, run=measure, jobs: int = 1, base_env=(), head_env=()):
    """Rows (cell, level, base, head, verdict) for every cell both builds measured. `run` is `measure`."""
    work = [(b, src, region, level, env) for src, region in cells for level in levels for b, env in ((base_bin, tuple(base_env)), (head_bin, tuple(head_env)))]
    results = list(Pool(jobs).imap(run, work)) if jobs > 1 else [run(w) for w in work]
    rows = []
    for i in range(0, len(work), 2):
        base, head = results[i], results[i + 1]
        cell, level = work[i][1], work[i][3]
        if isinstance(base, str) or isinstance(head, str):
            rows.append((cell, level, base, head, "unmeasured"))
        elif base[2] != head[2]:
            rows.append((cell, level, base, head, "output differs"))
        else:
            rows.append((cell, level, base[:2], head[:2], verdict(level, rate, base[:2], head[:2])))
    return rows


def main(argv=None) -> int:
    if argv is None and sys.argv[1:2] == ["--measure"]:
        print(json.dumps(measure_here(tuple(json.loads(sys.argv[2])))))
        return 0
    ap = argparse.ArgumentParser()
    ap.add_argument("base")
    ap.add_argument("head")
    ap.add_argument("--levels", default=",".join(LEVELS))
    ap.add_argument("--only", default="")
    ap.add_argument("--jobs", type=int, default=2)
    ap.add_argument("--json", default="")
    ap.add_argument("--base-env", action="append", default=[], metavar="NAME=VALUE")
    ap.add_argument("--head-env", action="append", default=[], metavar="NAME=VALUE")
    args = ap.parse_args(argv)
    sys.path[:0] = [str(ROOT / "tools/bench")]
    os.environ.setdefault("LLRM_BIN", args.base)  # bench reads it at import, for the rate only
    import bench

    cells = programs(ROOT / "bench", set(args.only.split(",")) if args.only else None)
    rows = compare(args.base, args.head, cells, args.levels.split(","), bench.rate_for("c", "O2"), jobs=args.jobs, base_env=[e.split("=", 1) for e in args.base_env], head_env=[e.split("=", 1) for e in args.head_env])
    for level in args.levels.split(","):
        mine = [r for r in rows if r[1] == level]
        counts = {v: sum(r[4] == v for r in mine) for v in ("better", "same", "worse", "trade", "output differs", "unmeasured")}
        both = [r for r in mine if r[4] not in ("output differs", "unmeasured")]
        print(level, len(mine), "cells:", ", ".join(f"{n} {v}" for v, n in counts.items() if n), f"| head minus base: {sum(r[3][0] - r[2][0] for r in both):+d} B, {sum(r[3][1] - r[2][1] for r in both):+d} clk")
        for cell, _, base, head, v in mine:
            if v not in ("same",):
                print(f"  {cell.split('/')[1]:18} {v:15} {base} -> {head}")
    if args.json:
        Path(args.json).write_text(json.dumps(rows))
    return 1 if any(r[4] in ("output differs", "unmeasured") for r in rows) else 0


if __name__ == "__main__":
    sys.exit(main())
