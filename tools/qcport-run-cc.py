#!/usr/bin/env python3
"""QCport built through llrm-cc, run against the all-Borland build: the clang front end's gate.

    QCPORT=.../src QCPORT_INC=... QCPORT_BORLAND=DIR [QCPORT_LLRMC_MODULES="dbg sys_time d_poly ent"] tools/qcport-run-cc.py [-Os]

Every module of QCport's six source directories is compiled by llrm-cc for x86_16-dos (LLRM_CLANG_M16 names the i8086 clang;
include path: LLRM_BORLAND_INC, made by tools/lowercase-includes.sh), except QCPORT_LLRMC_MODULES, which llrm-c compiles:
Borland's `_asm` is not carried yet. The objects replace the Borland build's in QCPORT.LNK; both builds run headless as
tools/qcport-run.py runs them and must draw the same frames, polygons and BENCH.BMP. Exit 1 on a build failure or a difference.
"""
from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("qcport_run", HERE / "qcport-run.py")
qr = importlib.util.module_from_spec(spec)
sys.modules["qcport_run"] = qr
spec.loader.exec_module(qr)
dosbatch = qr.dosbatch


def main() -> int:
    level = sys.argv[1] if len(sys.argv) > 1 else "-Os"
    qcport, include, borland = qr.required("QCPORT"), qr.required("QCPORT_INC"), qr.required("QCPORT_BORLAND")
    borland_include = qr.required("LLRM_BORLAND_INC")
    root = HERE.parent
    cc, llrm_c = root / "target/release/llrm-cc", qr.llrmbin.bin_dir() / "llrm-c"
    by_llrm_c = set(os.environ.get("QCPORT_LLRMC_MODULES", "dbg sys_time d_poly ent").split())
    sources = sorted(source for part in qr.MODULES for source in (qcport / part).glob("*.c"))
    dirs = [flag for part in (*qr.MODULES, "qgl") for flag in ("-I", str(qcport / part))]
    work = dosbatch.private_work("qcport-run-cc")
    sides = {"borland": work / "borland", "llrm": work / "llrm"}
    for side in sides.values():
        side.mkdir(parents=True)
        for one in [*borland.glob("*.obj"), *(borland / name for name in qr.DATA)]:
            if one.exists():
                shutil.copy(one, side / one.name)

    def compile_one(source: Path):
        out = sides["llrm"] / f"{source.stem}.obj"
        if source.stem in by_llrm_c:
            return qr.compile_failure(llrm_c, [*dirs, "-I", str(include)], source, out, level)
        env = {**os.environ, "LLRM_CC_TARGET": "x86_16-dos"}
        done = subprocess.run([str(cc), level, "-c", "-fasm-blocks", "-D_asm=__asm", "-nostdinc", "-isystem", str(borland_include), *dirs, str(source), "-o", str(out)], capture_output=True, text=True, env=env)
        return None if done.returncode == 0 else f"{source.stem}: {done.stderr.strip()[-400:]}"

    with ThreadPoolExecutor(max_workers=6) as pool:
        failed = [one for one in pool.map(compile_one, sources) if one]
    stand_in = subprocess.run([str(llrm_c), "-O1", "-fno-tree-loop-distribute-patterns", "-I", str(include), str(HERE / "qcport" / "strlib.c"), "-o", str(sides["llrm"] / "strlib.obj")], capture_output=True, text=True)
    if failed or stand_in.returncode:
        print("compile failed:", *failed, stand_in.stderr.strip(), sep="\n  ")
        return 1
    listing = (borland / "QCPORT.LNK").read_text()
    toolchain = Path(os.environ["QCPORT_BCC_LIB"]) if os.environ.get("QCPORT_BCC_LIB") else None
    linker = os.environ.get("JWLINK", "jwlink")
    for name, side in sides.items():
        (side / "QCPORT.LNK").write_text(qr.linked(listing, side, ["strlib.obj"] if name == "llrm" else [], toolchain))
        done = subprocess.run([linker, f"@{side / 'QCPORT.LNK'}"], capture_output=True, text=True)
        if not (side / "QCPORT.EXE").exists() or qr.re.search(r"undefined|Error!", done.stdout + done.stderr):
            print(f"{name}: link failed\n" + (done.stdout + done.stderr)[-800:])
            return 1
    cap, stall = float(os.environ.get("QCPORT_RUN_SECONDS", "900")), float(os.environ.get("QCPORT_STALL_SECONDS", "90"))
    with ThreadPoolExecutor() as pool:
        stopped = [why for why in pool.map(lambda side: qr.run(side, qcport, stall, cap), sides.values()) if why]
    if stopped:
        print(f"a run did not finish: {', '.join(stopped)} ({work})")
        return 1
    reference, built = qr.measured(sides["borland"]), qr.measured(sides["llrm"])
    differences = qr.verdict(reference, built)
    if differences:
        print("QCport built through llrm-cc draws differently from the Borland build:", *differences, f"({work})", sep="\n  ")
        print("\n".join(line for side in sides.values() for line in qr.tails(side)))
        return 1
    print(f"QCport: {len(sources)} modules ({len(by_llrm_c)} by llrm-c) draw what the Borland build draws: {built['frames']} frames, {built['polys']} polys, bmp {built['md5'][:8]}")
    dosbatch.discard(work)
    return 0


if __name__ == "__main__":
    sys.exit(main())
