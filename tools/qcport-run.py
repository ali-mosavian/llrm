#!/usr/bin/env python3
"""Run QCport built by llrm-c and compare it with the all-Borland build: the gate that executes what qcport-compile.sh compiles.

    QCPORT=~/scratch/qcport/src QCPORT_INC=~/scratch/qctc/inc QCPORT_BORLAND=~/scratch/qcbcc \\
        tools/qcport-run.py [llrm-c]

QCPORT names QCport's src directory (its tools/run.sh is beside it), QCPORT_INC its Borland headers, QCPORT_BORLAND a
build of it by Borland C: the objects, QCPORT.LNK, and the files a run reads (stuff.ini, default.cfg, start.qmp,
QUAKE.QPK, run.conf). JWLINK names the linker (default `jwlink`), DOSBOX_BIN DOSBox-X (default the tree's). QCPORT_OBJECTS
names a directory of the modules' objects already built by llrm-c -O2 (the gate's qcport-cmp.sh makes them): they are
used as they are and nothing is compiled, which saves the 40 s of compiling.

Both builds link the same objects but QCport's 65 C modules, which the second compiles with llrm-c -O2, and run headless
(`start.qmp -ticks 300`) in a work directory of their own. They must draw the same: the frames, the polygons and the
md5 of BENCH.BMP. Any difference, a run that does not finish in QCPORT_RUN_SECONDS (30), or a build that fails exits 1.
"""
from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "dosbatch"))

import dosbatch  # noqa: E402
import llrmbin  # noqa: E402

MODULES = ("host", "render", "model", "game", "sound", "ui")
DATA = ("stuff.ini", "default.cfg", "start.qmp", "QUAKE.QPK", "run.conf")
ARGUMENTS = "start.qmp -ticks 300"
# FP87 first: the order the build that runs uses; CM first leaves a program that links and stops after its arguments.
LIBRARIES = ("FP87", "MATHM", "CM")


def required(name: str) -> Path:
    value = os.environ.get(name)
    if not value:
        sys.exit(f"{name} is not set; see the top of {Path(__file__).name}")
    return Path(value).expanduser()


def linked(listing: str, objects: Path, extra: list[str]) -> str:
    """`listing` (a Borland build's QCPORT.LNK) naming the objects in `objects`, with `extra` added, libraries in the order that runs."""
    lines, libraries = [], []
    for line in listing.splitlines():
        found = re.match(r"file '(.*/)?([^/]+)\.obj'$", line, re.I)
        if line.startswith("library"):
            libraries.append(line)
        elif line.startswith(("option", "name")):
            continue
        elif found and found.group(2).upper() != "C0M":
            lines.append(f"file '{objects / (found.group(2) + '.obj')}'")
        else:
            lines.append(line)
    lines = ["format dos", f"option quiet, map={objects / 'QCPORT.MAP'}", f"name {objects / 'QCPORT.EXE'}", *[one for one in lines if not one.startswith("format")]]
    lines += [f"file '{objects / one}'" for one in extra]
    rank = lambda line: LIBRARIES.index(re.search(r"(\w+)\.LIB", line, re.I).group(1).upper())
    return "\n".join(lines + sorted(libraries, key=rank)) + "\n"


def measured(directory: Path) -> dict[str, str]:
    """What a run drew: frames, polys, and the md5 of its picture."""
    text = (directory / "BENCH.TXT").read_text() if (directory / "BENCH.TXT").exists() else ""
    found = dict(re.findall(r"^(frames|polys)\s+(\d+)", text, re.M))
    found["md5"] = hashlib.md5((directory / "BENCH.BMP").read_bytes()).hexdigest() if (directory / "BENCH.BMP").exists() else ""
    return found


def verdict(reference: dict[str, str], built: dict[str, str]) -> list[str]:
    """What differs between the two runs: empty where they drew the same. A run that drew nothing differs."""
    if not reference.get("polys") or not built.get("polys"):
        return [f"a run drew nothing: Borland {reference}, llrm {built}"]
    return [f"{key}: Borland {reference.get(key)}, llrm {built.get(key)}" for key in ("frames", "polys", "md5") if reference.get(key) != built.get(key)]


def run(directory: Path, qcport: Path, seconds: int) -> None:
    environment = {**os.environ, "DOSBOX_BIN": os.environ.get("DOSBOX_BIN", str(dosbatch.DOSBOX))}
    subprocess.run([str(qcport.parent / "tools" / "run.sh"), str(directory), ARGUMENTS], env=environment, capture_output=True, timeout=seconds, check=False)


def main() -> int:
    qcport, include, borland = required("QCPORT"), required("QCPORT_INC"), required("QCPORT_BORLAND")
    compiler = Path(sys.argv[1]) if len(sys.argv) > 1 else llrmbin.bin_dir() / "llrm-c"
    linker = os.environ.get("JWLINK", "jwlink")
    seconds = int(os.environ.get("QCPORT_RUN_SECONDS", "30"))
    work = dosbatch.private_work("qcport-run")
    sides = {"borland": work / "borland", "llrm": work / "llrm"}
    for side in sides.values():
        side.mkdir(parents=True)
        for one in [*borland.glob("*.obj"), *(borland / name for name in DATA)]:
            if one.exists():
                shutil.copy(one, side / one.name)
    sources = sorted(source for part in MODULES for source in (qcport / part).glob("*.c"))
    built_already = Path(os.environ["QCPORT_OBJECTS"]).expanduser() if os.environ.get("QCPORT_OBJECTS") else None
    flags = [flag for part in (*MODULES, "qgl") for flag in ("-I", str(qcport / part))] + ["-I", str(include)]

    def compile_one(source: Path):
        done = subprocess.run([str(compiler), "-O2", *flags, str(source), "-o", str(sides["llrm"] / f"{source.stem}.obj")], capture_output=True, text=True)
        return None if done.returncode == 0 else f"{source.stem}: {(done.stderr or done.stdout).strip()[-300:]}"

    if built_already:
        missing = [source.stem for source in sources if not (built_already / f"{source.stem}.obj").exists()]
        if missing:
            print(f"QCPORT_OBJECTS lacks: {' '.join(missing)}")
            return 1
        failed = []
        for source in sources:
            shutil.copy(built_already / f"{source.stem}.obj", sides["llrm"] / f"{source.stem}.obj")
    else:
        with ThreadPoolExecutor() as pool:
            failed = [one for one in pool.map(compile_one, sources) if one]
    stand_in = subprocess.run([str(compiler), "-O1", "-fno-tree-loop-distribute-patterns", str(HERE / "qcport" / "strlib.c"), "-o", str(sides["llrm"] / "strlib.obj")], capture_output=True, text=True)
    if failed or stand_in.returncode:
        print("compile failed:", *failed, stand_in.stderr.strip(), sep="\n  ")
        return 1
    listing = (borland / "QCPORT.LNK").read_text()
    for name, side in sides.items():
        (side / "QCPORT.LNK").write_text(linked(listing, side, ["strlib.obj"] if name == "llrm" else []))
        done = subprocess.run([linker, f"@{side / 'QCPORT.LNK'}"], capture_output=True, text=True)
        if not (side / "QCPORT.EXE").exists() or re.search(r"undefined|Error!", done.stdout + done.stderr):
            print(f"{name}: link failed\n" + (done.stdout + done.stderr)[-600:])
            return 1
    with ThreadPoolExecutor() as pool:
        try:
            list(pool.map(lambda side: run(side, qcport, seconds), sides.values()))
        except subprocess.TimeoutExpired:
            print(f"a run did not finish in {seconds}s ({work})")
            return 1
    reference, built = measured(sides["borland"]), measured(sides["llrm"])
    differences = verdict(reference, built)
    if differences:
        print("QCport built by llrm-c draws differently from the Borland build:", *differences, f"({work})", sep="\n  ")
        return 1
    print(f"QCport: {len(sources)} modules by llrm-c draw what the Borland build draws: {built['frames']} frames, {built['polys']} polys, bmp {built['md5'][:8]}")
    dosbatch.discard(work)
    return 0


if __name__ == "__main__":
    sys.exit(main())
