#!/usr/bin/env python3
"""Open Watcom's clib self-tests (`bld/clib/qa`), built by llrm-c against the library and by the pinned
compiler against Open Watcom's own, run under DOSBox-X; the two runs compared.

    qa.py --ow SRC --work DIR --llrm-c C --jwlink L --lib LIB --reference BINBUILD --wlink W
          --ref-lib CLIBM --headers H [--only NAME]
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent / "dosbatch"))
sys.path.insert(0, str(HERE.parent))


def sources(ow):
    clib = ow / "bld" / "clib"
    tests = ow / "bld" / "clibtest"
    found = {}
    for makefile in sorted((clib / "qa").glob("*/makefile")):
        m = re.search(r"srcfile\s*=\s*\$\(clib_dir\)/(\S+)", makefile.read_text())
        if m and (tests / m[1]).exists():
            found[makefile.parent.name] = tests / m[1]
    return found


def includes(ow, headers, source):
    clib = ow / "bld" / "clib"
    bld = ow / "bld"
    dirs = [source.parent.parent / "h", source.parent, ow / "bld" / "clibtest" / "h", clib / "h", headers, bld / "watcom" / "h", bld / "lib_misc" / "h",
            bld / "comp_cfg" / "h", clib / "intel" / "h", clib / "startup" / "h", clib / "streamio" / "h"]
    return [f"-I{d}" for d in dirs if d.exists()]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ow", type=Path, required=True)
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--llrm-c", type=Path, required=True)
    ap.add_argument("--jwlink", type=Path, required=True)
    ap.add_argument("--lib", type=Path, required=True)
    ap.add_argument("--reference", type=Path, required=True)
    ap.add_argument("--wlink", type=Path, required=True)
    ap.add_argument("--ref-lib", type=Path, required=True)
    ap.add_argument("--headers", type=Path, required=True)
    ap.add_argument("--only", default="")
    ap.add_argument("flags", nargs="*")
    a = ap.parse_args()
    import dosbatch

    shutil.rmtree(a.work, ignore_errors=True)
    a.work.mkdir(parents=True)
    jobs, built = [], {}
    for name, source in sources(a.ow).items():
        if a.only not in name:
            continue
        stem = re.sub(r"[^a-z0-9]", "", name)[:6]
        for kind in ("ours", "ref"):
            obj, exe = a.work / f"{stem}{kind[0]}.obj", a.work / f"{stem}{kind[0]}.exe"
            inc = includes(a.ow, a.headers, source)
            if kind == "ours":
                cc = [str(a.llrm_c), "-m16", "-mabi=watcom", "-fobject-format=omf", *a.flags, *inc, "-o", str(obj), str(source)]
                ln = [str(a.jwlink), "format", "dos", "option", "quiet", "option", "stack=5k", "reference", "_cstart_",
                      "file", str(obj), "library", str(a.lib), "name", str(exe)]
            else:
                cc = [str(a.reference / "bwcc"), "-zq", "-wx", "-oaxt", "-fpc", "-mm", "-bt=dos", *inc, "-fo=" + str(obj), str(source)]
                ln = [str(a.wlink), "format", "dos", "option", "quiet", "option", "stack=5k", "file", str(obj),
                      "library", str(a.ref_lib), "name", str(exe)]
            made = subprocess.run(cc, capture_output=True, text=True)
            if made.returncode != 0 or not obj.exists():
                built[(name, kind)] = ("compile failed", (made.stdout + made.stderr)[-400:])
                continue
            linked = subprocess.run(ln, stdin=subprocess.DEVNULL, capture_output=True, text=True,
                                    env={**os.environ, "WATCOM": str(a.ref_lib.parents[1])})
            if not exe.exists():
                built[(name, kind)] = ("link failed", (linked.stdout + linked.stderr)[-400:])
                continue
            built[(name, kind)] = ("built", "")
            jobs.append(dosbatch.Job(f"{stem}{kind[0]}", "exe", exe, budget_ms=60000))
            built[(name, kind, "job")] = f"{stem}{kind[0]}"
    results = dosbatch.run(jobs, a.work / "dos") if jobs else {}
    table = []
    for name in sorted({k[0] for k in built}):
        row = {}
        for kind in ("ours", "ref"):
            status = built.get((name, kind))
            if status and status[0] == "built":
                result = results[built[(name, kind, "job")]]
                # The program's own name is in what it prints; the two builds' differ.
                row[kind] = (result.status, result.exit_code, re.sub(r"c:\\\w+\.exe", "PROGRAM", result.text, flags=re.I))
            else:
                row[kind] = (status[0] if status else "-", None, status[1] if status else "")
        table.append((name, row))
    same = 0
    for name, row in table:
        o, r = row["ours"], row["ref"]
        verdict = "SAME" if o[:3] == r[:3] and o[0] == "ok" else ("both failed alike" if o[:2] == r[:2] else "DIFFERENT")
        same += verdict == "SAME"
        print(f"{name:12} ours={o[0]}/{o[1]} ref={r[0]}/{r[1]}  {verdict}")
        if verdict == "DIFFERENT":
            print("   ours:", o[2][-300:].replace("\n", " | "))
            print("   ref :", r[2][-300:].replace("\n", " | "))
    print(f"{same} of {len(table)} tests print the same and exit the same")


if __name__ == "__main__":
    main()
