#!/usr/bin/env python3
"""QCport built through llrm-cc, run against the all-Borland build: the clang front end's gate.

    QCPORT=.../src QCPORT_INC=... QCPORT_BORLAND=DIR [QCPORT_OBJECTS=DIR] tools/qcport-run-cc.py [-Os] [--row x86_16-dos-rp3]

Every module of QCport's six source directories is compiled by llrm-cc for the row --row names (default x86_16-dos, or QCPORT_CC_ROW; LLRM_CLANG_M16 names the i8086 clang;
include path: LLRM_BORLAND_INC, made by tools/lowercase-includes.sh; Borland's `_asm` goes through -fasm-blocks). QCPORT_OBJECTS names a directory
of the modules' objects already built (tools/oracle_qcport.py makes them with the oracles on): they are used as they are. The objects
replace the Borland build's in QCPORT.LNK; both builds run headless and must draw the same frames, polygons and BENCH.BMP. Then both play the
attract loop (an empty command line): all three demos. Exit 1 on a build failure or a difference.
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import qcport_run as qr  # noqa: E402

dosbatch = qr.dosbatch


def main() -> int:
    argv = sys.argv[1:]
    row = os.environ.get("QCPORT_CC_ROW", "x86_16-dos")
    if "--row" in argv:
        at = argv.index("--row")
        if at + 1 >= len(argv):
            sys.exit("--row wants a llrm-cc target row, e.g. x86_16-dos-rp3")
        row = argv[at + 1]
        del argv[at : at + 2]
    level = argv[0] if argv else "-Os"
    qr.required("LLRM_CLANG_M16")
    qcport, include, borland = qr.required("QCPORT"), qr.required("QCPORT_INC"), qr.required("QCPORT_BORLAND")
    borland_include = qr.required("LLRM_BORLAND_INC")
    root = HERE.parent
    cc = root / "target/release/llrm-cc"
    built_already = Path(os.environ["QCPORT_OBJECTS"]).expanduser() if os.environ.get("QCPORT_OBJECTS") else None
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
        done = cc_run(level, [*dirs, str(source)], out)
        return None if done.returncode == 0 else f"{source.stem}: {done.stderr.strip()[-400:]}"

    def cc_run(level, inputs, out):
        env = {**os.environ, "LLRM_CC_TARGET": row}
        return subprocess.run([str(cc), level, "-c", "-fasm-blocks", "-D_asm=__asm", "-nostdinc", "-isystem", str(borland_include), *inputs, "-o", str(out)], capture_output=True, text=True, env=env)

    if built_already:
        missing = [source.stem for source in sources if not (built_already / f"{source.stem}.obj").exists()]
        if missing:
            print(f"QCPORT_OBJECTS lacks: {' '.join(missing)}")
            return 1
        failed = []
        for source in sources:
            shutil.copy(built_already / f"{source.stem}.obj", sides["llrm"] / f"{source.stem}.obj")
    else:
        with ThreadPoolExecutor(max_workers=6) as pool:
            failed = [one for one in pool.map(compile_one, sources) if one]
    stand_in = cc_run("-O1", ["-fno-builtin", str(HERE / "qcport" / "strlib.c")], sides["llrm"] / "strlib.obj")
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
        stopped = [why for why in pool.map(lambda side: qr.run(side, qcport, stall, cap, over=qr.faults), sides.values()) if why]
    if stopped:
        print(f"a run did not finish: {', '.join(stopped)} ({work})")
        return 1
    reference, built = qr.measured(sides["borland"]), qr.measured(sides["llrm"])
    differences = qr.verdict(reference, built) + (qr.pinned(len(sources), built) if level == "-Os" else [])
    if differences:
        print("QCport built through llrm-cc draws differently from the Borland build:", *differences, f"({work})", sep="\n  ")
        print("\n".join(line for side in sides.values() for line in qr.tails(side)))
        return 1
    for side in sides.values():  # run.sh clears error.log, and the program writes ERROR.LOG: the benchmark run's would be read as the attract run's
        (side / "ERROR.LOG").unlink(missing_ok=True)
    attract_cap, attract_stall = float(os.environ.get("QCPORT_ATTRACT_SECONDS", "900")), float(os.environ.get("QCPORT_ATTRACT_STALL_SECONDS", "300"))  # loading a demo is silent for minutes at the pinned 75000 cycles
    with ThreadPoolExecutor() as pool:
        stopped = [why for why in pool.map(lambda side: qr.run(side, qcport, attract_stall, attract_cap, arguments=qr.ATTRACT_ARGUMENTS, over=qr.attract_over), sides.values()) if why]
    failures = qr.attract_verdict({name: qr.attract_problems(side) for name, side in sides.items()})
    if stopped or failures:
        print("the attract loop (empty command line) did not play all three demos:", *failures, *stopped, f"({work})", sep="\n  ")
        print("\n".join(line for side in sides.values() for line in qr.tails(side)))
        return 1
    print(f"QCport: {len(sources)} modules at {level} draw what the Borland build draws: {built['frames']} frames, {built['polys']} polys, bmp {built['md5'][:8]}; the attract loop plays demo1-3")
    dosbatch.discard(work)
    return 0


if __name__ == "__main__":
    sys.exit(main())
