#!/usr/bin/env python3
"""Compile Open Watcom's clib for a 16-bit model with llrm-c; one JSON line per object.

    inventory.py --ow SRC --out DIR [--model mm] [--only SUBSTR] [-j N]

The object list is the library's own (`objects.mif` tags, `deps.mif` sources), the
flags are `flags.mif`'s. See docs/working/clib-build.md.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

SUBLIBS = (
    "alias bios cgsupp char conio convert direct environ file fpu handleio heap intel kanji math "
    "mbyte memory misc posix process search startup streamio string time _dos"
).split()


def objects(clib, tag="d16"):
    """(directory, object name, extra options, extra post options) of each object with `tag`."""
    for sub in SUBLIBS:
        mif = clib / sub / "objects.mif"
        if not mif.exists():
            continue
        text = mif.read_text(errors="replace")
        opts = {m[1]: m[2].strip() for m in re.finditer(r"^(\w+)_opts\s*=(.*)$", text, re.M)}
        post = {m[1]: m[2].strip() for m in re.finditer(r"^(\w+)_postopts\s*=(.*)$", text, re.M)}
        for line in text.splitlines():
            parts = line.split()
            if len(parts) > 2 and parts[0] == "!inject" and tag in parts[2:]:
                name = parts[1][: -len(".obj")]
                yield sub, name, opts.get(name, ""), post.get(name, "")


# The makefiles' ppflags_common: the build's version and copyright year, for the start-up's banner.
BUILD = ["-D_BLDVER=260", "-D_CYEAR=2026"]


def resolved(text):
    """The compiler switches in a makefile's `NAME_opts` value: `$($(processor)_option_X)` is `-X`, a
    `-I"$(clib_dir)/D/h"` is the include directory (`includes`), anything else spelled with a macro is dropped."""
    found = []
    for word in text.split():
        m = re.fullmatch(r"\$\(\$\(processor\)_(?:\$\(model\)_)?option_(\w+)\)", word)
        if m:
            found.append("-" + m[1])
        elif "$" not in word and not word.startswith("-I"):
            # `-zld-mt` is two switches, as wcc reads it.
            found.extend(re.findall(r"-[A-Za-z0-9=_.+]+", word) if "/" not in word else [word])
    return found


def source(clib, sub, name):
    deps = clib / sub / "deps.mif"
    if deps.exists():
        m = re.search(rf"^{re.escape(name)}\.obj\s*:\s*(\S+)", deps.read_text(errors="replace"), re.M)
        if m:
            path = (clib / sub / "library" / "msdos.086" / "mm" / m[1]).resolve()
            if path.exists():
                return path
    for kind in ("c", "a"):
        for ext in (".c", ".asm", ".gas"):
            path = clib / sub / kind / (name + ext)
            if path.exists():
                return path
    return None


def includes(ow, clib, sub, extra, headers):
    bld = ow / "bld"
    dirs = [clib / sub / "h", clib / "h", headers, bld / "watcom" / "h", bld / "lib_misc" / "h", bld / "comp_cfg" / "h"]
    found = [f"-I{d}" for d in dirs]
    for m in re.finditer(r'-I"?\$\(clib_dir\)/(\w+)/h"?', extra):
        found.append(f"-I{clib / m[1] / 'h'}")
    return found


def run(job, args):
    sub, name, opts, post = job
    clib = args.ow / "bld" / "clib"
    src = source(clib, sub, name)
    record = {"dir": sub, "obj": name, "src": str(src.relative_to(args.ow)) if src else None,
              "standalone": "-zld" in post.split()}
    if src is None:
        return {**record, "status": "nosource"}
    out = args.out / "obj" / sub / f"{name}.obj"
    out.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    if args.reference and src.suffix == ".c":
        cmd = [str(args.reference / "bwcc"), "-zq", "-we", "-j", "-zastd=c99", "-zl", "-x", "-xx", "-zam", "-wpx", "-s",
               "-oabelr", "-om", *args.ref_flags, "-mm", "-nt=_TEXT", "-fpc", "-0", "-bt=dos", *resolved(opts), *resolved(post),
               *includes(args.ow, clib, sub, opts, args.headers), "-fo=" + str(out), str(src)]
        tool = "bwcc"
    elif args.reference:
        cmd = [str(args.reference / "bwasm"), "-q", "-we", "-w8", "-cx", "-ee", "-fpc", "-0", "-mm", "-nt=_TEXT", "-bt=dos", *BUILD,
               *resolved(opts), *resolved(post), *includes(args.ow, clib, sub, opts, args.headers), "-fo=" + str(out), str(src)]
        tool = "bwasm"
    elif src.suffix == ".c":
        # The makefile's -os asks for size, for this object alone.
        size = ["-Os"] if "-os" in resolved(opts) + resolved(post) else []
        cmd = [str(args.llrm_c), "-m16", "-mabi=watcom", "-fobject-format=omf", *args.flags, *size,
               *includes(args.ow, clib, sub, opts, args.headers), "-o", str(out), str(src)]
        tool = "llrm-c"
    else:
        cmd = [str(args.jwasm), "-q", "-omf", "-Cp", "-Zg", "-Zm", "-0", "-mt" if "-mt" in resolved(opts) + resolved(post) else "-mm", "-D__WASM__", "-D__TINY__" if "-mt" in resolved(opts) + resolved(post) else "-D__MEDIUM__",
               "-D__FPI87__" if "-fpi87" in resolved(opts) + resolved(post) else "-D__FPC__", "-D__DOS__", "-D_M_I86", *BUILD, "-Fo" + str(out),
               *includes(args.ow, clib, sub, opts, args.headers), str(src)]
        tool = "jwasm"
    done = subprocess.run(cmd, capture_output=True, text=True, cwd=args.out, errors="replace")
    if done.returncode != 0 and tool == "jwasm" and "Segment definition changed" in done.stdout + done.stderr:
        # d16msels redefines _DATA with its own alignment; wasm takes it, JWasm refuses it after a memory model.
        cmd = [word for word in cmd if word not in ("-mm", "-mt")]
        done = subprocess.run(cmd, capture_output=True, text=True, cwd=args.out, errors="replace")
        record["note"] = "assembled without a memory model switch"
    seconds = time.monotonic() - started
    ok = done.returncode == 0 and out.exists()
    if not ok and out.exists():
        out.unlink()
    return {**record, "tool": tool, "status": "ok" if ok else "fail", "seconds": round(seconds, 3),
            "bytes": out.stat().st_size if ok else 0,
            "error": "" if ok else (done.stdout + done.stderr).strip()[-1500:]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ow", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--only", default="")
    ap.add_argument("-j", type=int, default=os.cpu_count())
    ap.add_argument("--llrm-c", type=Path)
    ap.add_argument("--jwasm", type=Path)
    ap.add_argument("--reference", type=Path, help="Open Watcom's own bwcc and bwasm directory: build with them instead")
    ap.add_argument("--headers", type=Path, required=True, help="installed Open Watcom h directory")
    ap.add_argument("--ref-flag", dest="ref_flags", action="append", default=[], help="extra bwcc flag, e.g. -os")
    ap.add_argument("flags", nargs="*")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    jobs = [j for j in objects(args.ow / "bld" / "clib") if args.only in f"{j[0]}/{j[1]}"]
    with ThreadPoolExecutor(args.j) as pool, open(args.out / "results.jsonl", "w") as log:
        for record in pool.map(lambda j: run(j, args), jobs):
            log.write(json.dumps(record) + "\n")
    print(len(jobs), "objects", file=sys.stderr)


if __name__ == "__main__":
    main()
