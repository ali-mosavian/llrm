#!/usr/bin/env python3
"""QCport's 65 modules at -Os: code bytes through llrm-cc, per module and in total.

    QCPORT=.../src QCPORT_INC=... LLRM_BORLAND_INC=... LLRM_CLANG_M16=... LLRM_OW=... \\
        tools/qcport-size.py [--row x86_16-dos-rp3] [--isel dag] [--top 12]

--row is the llrm-cc target row (x86_16-dos is cdecl, the Borland-link row). Prints the total and the largest modules;
a module that does not compile is left out of the total and named.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import importlib.util

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("sizes", ROOT / "tools/sizes.py")
sizes = importlib.util.module_from_spec(spec)
_argv, sys.argv = sys.argv, ["sizes"]
spec.loader.exec_module(sizes)
sys.argv = _argv
PARTS = "host render game model sound ui".split()


def need(name: str) -> str:
    if not os.environ.get(name):
        sys.exit(f"qcport-size: {name} is not set")
    return os.environ[name]


def main() -> int:
    ask = argparse.ArgumentParser()
    ask.add_argument("--row", default="x86_16-dos-rp3")
    ask.add_argument("--isel")
    ask.add_argument("--top", type=int, default=12)
    args = ask.parse_args()
    qcport, include, borland = Path(need("QCPORT")), need("QCPORT_INC"), need("LLRM_BORLAND_INC")
    need("LLRM_CLANG_M16"), need("LLRM_OW")
    bin_dir = ROOT / "target/release"
    extra = [f"--isel={args.isel}"] if args.isel else []
    dirs = [flag for part in (*PARTS, "qgl") for flag in ("-I", str(qcport / part))]
    modules = [source for part in PARTS for source in sorted((qcport / part).glob("*.c"))]
    with __import__("tempfile").TemporaryDirectory() as scratch:
        work = Path(scratch)

        def one(source: Path):
            cc = work / f"{source.stem}.cc.o"
            env = {**os.environ, "LLRM_CC_TARGET": args.row}
            subprocess.run(
                [str(bin_dir / "llrm-cc"), "-Os", "-c", *extra, "-fasm-blocks", "-D_asm=__asm", "-nostdinc", "-isystem", borland, *dirs, str(source), "-o", str(cc)],
                capture_output=True, env=env)
            return f"{source.parent.name}/{source.name}", sizes.code_bytes(cc) if cc.exists() else None

        with ThreadPoolExecutor(6) as pool:
            rows = list(pool.map(one, modules))
    built = [row for row in rows if row[1] is not None]
    skipped = [row[0] for row in rows if row[1] is None]
    print(f"row {args.row}{' --isel=' + args.isel if args.isel else ''}: {len(built)} modules, llrm-cc {sum(row[1] for row in built)} B")
    if skipped:
        print("left out (did not compile):", *skipped)
    for name, size in sorted(built, key=lambda row: -row[1])[: args.top]:
        print(f"  {name:28} {size:7}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
