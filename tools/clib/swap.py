#!/usr/bin/env python3
"""Which llrm-c module breaks a program: swap each one it links for the reference build's, one at a time.

    swap.py WORK OURS_DIR REF_DIR LIB_DIR PROGRAM.c --wlink W --wlib W --llrm-c C --headers H

Builds a library per swap, links PROGRAM against it, runs them all in one DOSBox launch
and prints the swaps whose output changed to the reference program's.
"""
import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
sys.path.insert(0, str(Path(__file__).parents[1] / "dosbatch"))
sys.path.insert(0, str(Path(__file__).parents[1]))
import omf  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("work", type=Path)
    ap.add_argument("ours", type=Path)
    ap.add_argument("ref", type=Path)
    ap.add_argument("lib", type=Path)
    ap.add_argument("program", type=Path)
    ap.add_argument("--wlink", type=Path, required=True)
    ap.add_argument("--wlib", type=Path, required=True)
    ap.add_argument("--llrm-c", type=Path, required=True)
    ap.add_argument("--headers", type=Path, required=True)
    ap.add_argument("--expect", type=Path, required=True, help="the output the program must print")
    a = ap.parse_args()
    import dosbatch

    work = a.work
    shutil.rmtree(work, ignore_errors=True)
    (work / "libs").mkdir(parents=True)
    manifest = json.loads((a.lib / "manifest.json").read_text())
    members = a.lib / "members"
    obj = work / "prog.obj"
    subprocess.run([str(a.llrm_c), "-m16", "-mabi=watcom", "-std=c99", "-fobject-format=omf", f"-I{a.headers}", "-o", str(obj), str(a.program)], check=True)
    link = lambda lib, exe, mapf: subprocess.run(
        [str(a.wlink), "format", "dos", "option", "quiet", f"option", f"map={mapf}", "reference", "_cstart_", "file", str(obj), "library", str(lib), "name", str(exe)],
        stdin=subprocess.DEVNULL, capture_output=True)
    link(a.lib / "clibllrm.lib", work / "base.exe", work / "base.map")
    mapped = set(re.findall(r"^[0-9a-f]{4}:[0-9a-f]{4}\*?\s+(\S+)$", (work / "base.map").read_text(), re.M))
    swaps = []
    for key, origin in manifest.items():
        if origin != "llrm-c":
            continue
        d, name = key.split("/")
        theirs = a.ref / "obj" / d / f"{name}.obj"
        if not theirs.exists() or not (omf.read(members / f"{name}.obj")["publics"] & mapped):
            continue
        swaps.append((name, theirs))
    jobs = []
    for at, (name, theirs) in enumerate(swaps):
        w = work / "libs" / name
        shutil.copytree(members, w / "m")
        shutil.copy(theirs, w / "m" / f"{name}.obj")
        (w / "cmd.lbc").write_text("".join(f"+{p.name}\n" for p in sorted((w / "m").glob("*.obj"))))
        lib = (w / "x.lib").resolve()
        subprocess.run([str(a.wlib), "-q", "-b", "-n", "-fo", str(lib), "@../cmd.lbc"], cwd=w / "m", check=True)
        link(lib, w / f"s{at}.exe", w / "s.map")
        if (w / f"s{at}.exe").exists():
            jobs.append((name, dosbatch.Job(f"s{at}", "exe", w / f"s{at}.exe", budget_ms=20000)))
        shutil.rmtree(w / "m")
    base = dosbatch.Job("base", "exe", work / "base.exe", budget_ms=20000)
    results = dosbatch.run([base] + [j for _, j in jobs], work / "dos")
    want = a.expect.read_text()
    print("base:", "PASS" if results["base"].text == want else "FAIL")
    for name, job in jobs:
        if results[job.stem].text == want:
            print("fixes:", name)
    print(len(jobs), "swaps tried")


if __name__ == "__main__":
    main()
