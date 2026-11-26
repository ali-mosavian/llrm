#!/usr/bin/env python3
"""QuickrBasic host-build clock harness: every column (ours, gcc, clang) is compiled with `tools/external_flags.py`'s set.

  qbclock.py build ARCH LEVEL VARIANT DIST SRC OUT   copy SRC to OUT and make build/hostperf/basic (VARIANT ours|gcc|clang; ARCH m32|m64|arm64)
  qbclock.py bench OUT_PREFIX...                      perf stat each OUT_PREFIX-{ours,gcc,clang}/build/hostperf/basic over tools/bench/*.bas
  qbclock.py count EXE PROG                           guest instructions of an arm64 EXE on PROG (qemu-aarch64, one instruction per block)
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
import qbrow  # noqa: E402
from external_flags import flags as external_flags  # noqa: E402

# The bar compiler: baselines name gcc-15 (15.2); plain `gcc` is the distro's 13.4, which does not inline what 15 does (QB arrays.bas ran 2.6x the cycles).
GCC = os.environ.get("QB_GCC", "gcc-15")
ARM64_GCC = os.environ.get("QB_ARM64_GCC", "aarch64-linux-gnu-gcc-15")
# The perf build (no EXEC_COUNT counters, no -g): what the clock bars measure; `build/host` is the -O1 test build.
BUILD_TARGET = "build/hostperf/basic"
MULTIARCH = ("-isystem", "/usr/include/x86_64-linux-gnu")


def gcc_major(gcc=GCC):
    try:
        return int(subprocess.run([gcc, "-dumpversion"], capture_output=True, text=True).stdout.split(".")[0])
    except (ValueError, OSError):
        return None


def bar_flags(arch, major=None):
    """The flags every column's compile line carries, after the make's own."""
    if arch == "arm64":
        return [*external_flags("arm64", ("-fPIE",), major)]
    reloc, cpu = (("-fPIE",), ()) if arch == "m64" else (("-fno-pic", "-fno-pie"), ("-march=i486",))
    return [f"-{arch}", *cpu, *(MULTIARCH if arch == "m32" else ()), *external_flags("x86", reloc, major)]


def clang_arm64_headers(clang, sysroot):
    """The pin's clang on the arm64-glibc row: its own headers, the cross glibc last (`-idirafter`) so the project's `-isystem` wins."""
    resource = subprocess.run([clang, "-print-resource-dir"], capture_output=True, text=True).stdout.strip()
    return ["--target=aarch64-linux-gnu", "-nostdinc", "-isystem", resource + "/include", "-idirafter", sysroot + "/usr/aarch64-linux-gnu/include"]


def refuse_cc(message):
    print(message, file=sys.stderr)
    return 1


def cc(argv, env=os.environ):
    arch, variant, dist, pin = env["QBA"], env["QBV"], env["QBDIST"], env.get("PIN_BIN", "")
    if "-c" in argv:
        # The product's perf build appends its own -O2 and -g: the level under test replaces them.
        argv = [one for one in argv if not (one == "-g" or one.startswith(("-O", "-W")))] + [f"-{env['QBL']}", "-w"]
    arm64 = arch == "arm64"
    gcc = ARM64_GCC if arm64 else GCC
    if "-c" not in argv:
        # arm64 runs under qemu-user: static, so no guest sysroot is needed at run time.
        return subprocess.call([gcc, "-static", *argv, "-latomic"] if arm64 else [gcc, f"-{arch}", "-no-pie", *argv, "-latomic"])
    if variant == "gcc":
        args = ["-std=c2x" if one == "-std=c23" else one for one in argv]
        # gcc -m32 -std=c2x rejects pfpu.c's `constexpr` doubles under x87 excess precision: the same flag the old column used.
        fast = ["-fexcess-precision=fast"] if arch == "m32" else []
        return subprocess.call([gcc, *args, *bar_flags(arch, gcc_major(gcc)), *fast, "-D__cdecl="])
    if variant == "clang":
        clang = os.path.join(pin, "clang")
        head = clang_arm64_headers(clang, env["LLRM_ARM64_SYSROOT"]) if arm64 else []
        return subprocess.call([clang, *head, *argv, *bar_flags(arch)])
    drop, kept = False, []
    for one in argv:
        if drop:
            drop = False
        elif one in ("-MF", "-MT"):
            drop = True
        elif one not in ("-MMD", "-MP"):
            kept.append(one)
    target = TARGETS[arch]
    ours = dict(env, LLRM_CC_TARGET=target)
    if not arm64:  # the arm64 llc is the one beside llrm-cc
        ours["LLRM_LLC"] = os.path.join(dist, "llrm-llc")
    made = subprocess.run([os.path.join(dist, "llrm-cc"), *kept, *bar_flags(arch), "-w"], env=ours, capture_output=True, text=True)
    if made.returncode:
        return refuse_cc(f"llrm-cc refused {kept[-1]}: {made.stderr.strip()[:300]}")
    return 0


TARGETS = {"m64": "x86_64-linux", "m32": "x86_32-linux", "arm64": "arm64-glibc"}


def build(arch, level, variant, dist, src, out):
    if variant == "ours":
        qbrow.require_hosted(os.path.join(dist, "llrm-cc"), TARGETS[arch], {} if arch == "arm64" else {"LLRM_LLC": os.path.join(dist, "llrm-llc")})
    shutil.rmtree(out, ignore_errors=True)
    shutil.copytree(src, out)
    wrapper = os.path.join(os.path.dirname(os.path.abspath(__file__)), "qbcc")
    env = dict(os.environ, QBA=arch, QBV=variant, QBDIST=dist, QBL=level)
    made = subprocess.run(
        ["make", "-j6", "-k", BUILD_TARGET, f"HOST_CC={wrapper}"],
        cwd=out,
        env=env,
        capture_output=True,
        text=True,
    )
    open(os.path.join(out, "make.log"), "w").write(made.stdout + made.stderr)
    ok = os.path.exists(os.path.join(out, BUILD_TARGET)) and made.returncode == 0
    print(f"{arch}-{level}-{variant}: {'built' if ok else 'FAILED (see make.log: a refused module fails the build)'}")
    return 0 if ok else 1


def count(exe, prog, cwd="."):
    """Guest instructions executed by an arm64 `exe prog` under qemu-user: one instruction per translation block, one log line per block."""
    run = subprocess.Popen(
        ["qemu-aarch64", "-one-insn-per-tb", "-d", "exec,nochain", "-D", "/dev/stdout", exe, prog],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        cwd=cwd,
    )
    mark, total, tail = b"Trace 0: ", 0, b""
    while chunk := run.stdout.read(1 << 22):
        data = tail + chunk
        total += data.count(mark)
        tail = data[-(len(mark) - 1) :]
    run.wait()
    return total


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
                    row[variant] = measure(f"{prefix}-{variant}/{BUILD_TARGET}", prog, scratch)
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
    sys.exit({"cc": lambda: cc(rest), "build": lambda: build(*rest), "bench": lambda: bench(rest), "count": lambda: print(count(*rest)) or 0}[command]())
