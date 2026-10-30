#!/usr/bin/env python3
"""Compile the corpus with LLRM_CHECK_IVUSERS=1: every carried IvUsers is rebuilt and compared; a stale one panics.

    tools/check-ivusers.py [llrm-c]     print the files that fail, exit 1 if any

Corpus and levels are compile-cost.py's (vsgcc programs, plus QCport when QCPORT and QCPORT_INC are set).
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import importlib.util  # noqa: E402

import llrmbin  # noqa: E402

spec = importlib.util.spec_from_file_location("compile_cost", HERE / "compile-cost.py")
compile_cost = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compile_cost)


def compile_one(job: tuple[str, list[str]]) -> tuple[str, str]:
    key, command = job
    done = subprocess.run(command, env={**os.environ, "LLRM_CHECK_IVUSERS": "1"}, capture_output=True, text=True)
    return key, "" if done.returncode == 0 else done.stderr[-300:]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "4")))
    parser.add_argument("compiler", nargs="?", type=Path)
    args = parser.parse_args()
    compiler = args.compiler or llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory(dir=compile_cost.SCRATCH) as tmp:
        work = Path(tmp)
        files = enumerate(compile_cost.files(work).items())
        jobs = [(f"{name} {level}", [str(compiler), level, *flags, str(source), "-S", "-o", str(work / f"{n}.s")]) for n, (name, (source, flags)) in files for level in compile_cost.LEVELS]
        with ThreadPoolExecutor(args.jobs) as pool:
            failed = [(k, why) for k, why in pool.map(compile_one, jobs) if why]
    for key, why in failed:
        print(f"{key}: {why.strip().splitlines()[-1] if why.strip() else 'failed'}")
    print(f"{len(jobs)} compiles, {len(failed)} failed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
