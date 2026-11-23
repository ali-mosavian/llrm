#!/usr/bin/env python3
"""QuickrBasic host-build clock harness: every column (ours, gcc, clang) is compiled with `tools/external_flags.py`'s set.

  qbclock.py build ARCH LEVEL VARIANT DIST SRC OUT   copy SRC to OUT and make build/host/basic (VARIANT ours|gcc|clang; ARCH m32|m64)
  qbclock.py bench OUT_PREFIX...                      perf stat each OUT_PREFIX-{ours,gcc,clang}/build/host/basic over tools/bench/*.bas
  qbclock.py cc ARGS...                               the make's HOST_CC (env QBV, QBA, QBDIST, PIN_BIN)

A module llrm-cc refuses fails the build: there is no gcc fallback, so a refused module cannot read as 'unchanged'.
The m32 column is an i486 everywhere (-march=i486): gcc defaults to i686 and clang to pentium4, which have cmov and SSE2.
"""

import glob
import math
import os
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
from external_flags import flags as external_flags  # noqa: E402

MULTIARCH = ("-isystem", "/usr/include/x86_64-linux-gnu")


def gcc_major(gcc="gcc"):
    try:
        return int(subprocess.run([gcc, "-dumpversion"], capture_output=True, text=True).stdout.split(".")[0])
    except (ValueError, OSError):
        return None


def bar_flags(arch, major=None):
    """The flags every column's compile line carries, after the make's own."""
    reloc, cpu = (("-fPIE",), ()) if arch == "m64" else (("-fno-pic", "-fno-pie"), ("-march=i486",))
    return [f"-{arch}", *cpu, *(MULTIARCH if arch == "m32" else ()), *external_flags("x86", reloc, major)]


def refuse_cc(message):
    print(message, file=sys.stderr)
    return 1


def cc(argv, env=os.environ):
    arch, variant, dist, pin = env["QBA"], env["QBV"], env["QBDIST"], env.get("PIN_BIN", "")
    if "-c" not in argv:
        return subprocess.call(["gcc", f"-{arch}", "-no-pie", *argv, "-latomic"])
    if variant == "gcc":
        args = ["-std=c2x" if one == "-std=c23" else one for one in argv]
        return subprocess.call(["gcc", *args, *bar_flags(arch, gcc_major()), "-D__cdecl="])
    if variant == "clang":
        return subprocess.call([os.path.join(pin, "clang"), *argv, *bar_flags(arch)])
    drop, kept = False, []
    for one in argv:
        if drop:
            drop = False
        elif one in ("-MF", "-MT"):
            drop = True
        elif one not in ("-MMD", "-MP"):
            kept.append(one)
    ours = dict(env, LLRM_CC_TARGET="x86_64-linux" if arch == "m64" else "x86_32-linux", LLRM_LLC=os.path.join(dist, "llrm-llc"))
    made = subprocess.run([os.path.join(dist, "llrm-cc"), *kept, *bar_flags(arch), "-w"], env=ours, capture_output=True, text=True)
    if made.returncode:
        return refuse_cc(f"llrm-cc refused {kept[-1]}: {made.stderr.strip()[:300]}")
    return 0


def build(arch, level, variant, dist, src, out):
    shutil.rmtree(out, ignore_errors=True)
    shutil.copytree(src, out)
    cflags = f"-std=c23 -D_DEFAULT_SOURCE -{level} -DARENA_AT_ZERO=0 -DPFIL_PATH=4096 -DEXEC_COUNT -Isrc/plat/host/include -Isrc -Isrc/core"
    wrapper = os.path.join(os.path.dirname(os.path.abspath(__file__)), "qbcc")
    env = dict(os.environ, QBA=arch, QBV=variant, QBDIST=dist)
    made = subprocess.run(
        ["make", "-j6", "-k", "build/host/basic", f"HOST_CC={wrapper}", f"HOST_CFLAGS={cflags}"],
        cwd=out,
        env=env,
        capture_output=True,
        text=True,
    )
    open(os.path.join(out, "make.log"), "w").write(made.stdout + made.stderr)
    ok = os.path.exists(os.path.join(out, "build/host/basic")) and made.returncode == 0
    print(f"{arch}-{level}-{variant}: {'built' if ok else 'FAILED (see make.log: a refused module fails the build)'}")
    return 0 if ok else 1


def measure(exe, prog, cwd):
    run = subprocess.run(
        ["perf", "stat", "-x,", "-e", "instructions:u,cycles:u", "-r", "5", exe, prog],
        capture_output=True,
        text=True,
        cwd=cwd,
        timeout=300,
    )
    got = {}
    for line in run.stderr.splitlines():
        part = line.split(",")
        if len(part) > 3 and part[2] in ("instructions:u", "cycles:u"):
            try:
                got[part[2]] = float(part[0])
            except ValueError:
                pass
    return got.get("cycles:u"), got.get("instructions:u")


def bench(prefixes):
    for prefix in prefixes:
        rows = {}
        for prog in sorted(glob.glob(f"{prefix}-gcc/tools/bench/*.bas")):
            row = {}
            for variant in ("ours", "gcc", "clang"):
                scratch = tempfile.mkdtemp()
                try:
                    row[variant] = measure(f"{prefix}-{variant}/build/host/basic", prog, scratch)
                finally:
                    shutil.rmtree(scratch, ignore_errors=True)
            rows[os.path.basename(prog)] = row
            print(prefix, os.path.basename(prog), row, flush=True)
        for other in ("gcc", "clang"):
            pairs = [(r["ours"], r[other]) for r in rows.values() if r["ours"][0] and r[other][0]]
            if pairs:
                cycles = math.exp(sum(math.log(a[0] / b[0]) for a, b in pairs) / len(pairs))
                insns = math.exp(sum(math.log(a[1] / b[1]) for a, b in pairs) / len(pairs))
                print(prefix, f"GEOMEAN ours/{other}: cycles {cycles:.4f} instructions {insns:.4f} over {len(pairs)} programs")
    return 0


if __name__ == "__main__":
    command, rest = sys.argv[1], sys.argv[2:]
    sys.exit({"cc": lambda: cc(rest), "build": lambda: build(*rest), "bench": lambda: bench(rest)}[command]())
