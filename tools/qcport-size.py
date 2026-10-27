#!/usr/bin/env python3
"""QCport's 65 modules at -Os: code bytes through llrm-c and through llrm-cc, per module and in total.

    QCPORT=.../src QCPORT_INC=... LLRM_BORLAND_INC=... LLRM_CLANG_M16=... LLRM_OW=... \\
        tools/qcport-size.py [--row x86_16-dos-rp3] [--isel dag] [--top 12]

--row is the llrm-cc target row (default x86_16-dos-rp3, the register convention llrm-c has at m16, so the two columns compare like
with like; x86_16-dos is cdecl, the Borland-link row). --isel is given to both compilers. Prints the totals and the modules where
llrm-cc is larger by most; a module either side cannot compile is left out of the totals and named.
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
            c, cc = work / f"{source.stem}.c.o", work / f"{source.stem}.cc.o"
            subprocess.run([str(bin_dir / "llrm-c"), "-Os", *extra, *dirs, "-I", include, str(source), "-o", str(c)], capture_output=True)
            env = {**os.environ, "LLRM_CC_TARGET": args.row}
            subprocess.run(
                [str(bin_dir / "llrm-cc"), "-Os", "-c", *extra, "-fasm-blocks", "-D_asm=__asm", "-nostdinc", "-isystem", borland, *dirs, str(source), "-o", str(cc)],
                capture_output=True, env=env)
            return f"{source.parent.name}/{source.name}", sizes.code_bytes(c) if c.exists() else None, sizes.code_bytes(cc) if cc.exists() else None

        with ThreadPoolExecutor(6) as pool:
            rows = list(pool.map(one, modules))
    both = [row for row in rows if row[1] is not None and row[2] is not None]
    skipped = [row[0] for row in rows if row not in both]
    c, cc = sum(row[1] for row in both), sum(row[2] for row in both)
    print(f"row {args.row}{' --isel=' + args.isel if args.isel else ''}: {len(both)} modules, llrm-c {c} B, llrm-cc {cc} B, {cc - c:+d} ({(cc - c) * 100 / c:+.2f}%)")
    if skipped:
        print("left out (one side did not compile):", *skipped)
    for name, a, b in sorted(both, key=lambda row: row[1] - row[2])[: args.top]:
        print(f"  {name:28} {a:7} {b:7} {b - a:+d}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
