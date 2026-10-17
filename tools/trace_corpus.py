#!/usr/bin/env python3
"""Record `LLRM_TRACE` over a corpus: d_faces ($QCPORT) and the bench programs, at -O1/-O2/-Os, in parallel, into one file.

    QCPORT=<qb-quake src> tools/trace_corpus.py out.jsonl [--llrm llrm-c] [--levels O1,O2,Os] [--jobs 8]

Every line of the trace carries the compile it was made for in `file`: `<source>@<level>`. Compiles append to the one file in one write each, so parallel compiles do not interleave their lines.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KEYWORDS = [f"-D{k}=" for k in "far huge near pascal cdecl __far _far __near _near".split()]


def corpus() -> list[tuple[str, list[str]]]:
    found = [(str(path), []) for path in sorted((ROOT / "bench").glob("*/*.c"))]
    if os.environ.get("QCPORT"):
        qc = Path(os.environ["QCPORT"]).expanduser()
        includes = [flag for d in ("", "render", "game", "host", "qgl") for flag in ("-I", str(qc / d))]
        found.append((str(qc / "render/d_faces.c"), KEYWORDS + includes))
    return found


def record(llrm: str, trace: Path, level: str, source: str, extra: list[str]) -> bool:
    done = subprocess.run(
        [llrm, "-m32", "-march=i486", f"-{level}", *extra, "-o", "/dev/null", f"{source}"],
        capture_output=True, text=True, env={**os.environ, "LLRM_TRACE": str(trace)},
    )
    return done.returncode == 0


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("trace")
    parser.add_argument("--llrm", default=str(ROOT / "target/release/llrm-c"))
    parser.add_argument("--levels", default="O1,O2,Os")
    parser.add_argument("--jobs", type=int, default=8)
    args = parser.parse_args()
    trace = Path(args.trace)
    trace.unlink(missing_ok=True)
    jobs = [(level, source, extra) for level in args.levels.split(",") for source, extra in corpus()]
    with ThreadPoolExecutor(args.jobs) as pool:
        results = list(pool.map(lambda job: (job, record(args.llrm, trace, *job)), jobs))
    failed = [job[1] for job, ok in results if not ok]
    print(f"{len(jobs) - len(failed)} of {len(jobs)} compiles traced into {trace}")
    if failed:
        print("failed: " + ", ".join(sorted(set(Path(one).name for one in failed))), file=sys.stderr)


if __name__ == "__main__":
    main()
