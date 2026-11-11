#!/usr/bin/env python3
"""The compiler-rt members a 6502 program links, built from runtime/compiler-rt as a linker would pick them from an archive: one object per
source file, pulled by an undefined symbol (`tests/rt_index.sh` says which file defines it). Static helpers stay inside their file's object.
Groups (tests/fp_sources.sh): int64 (one routine per file), f32, f32_64, f64, clz (__clzsi2, called by the -O2 6502 config). int32.c is not linked: the compiler calls the 32-bit helpers
in mos_rt.s with their own convention."""
import re, subprocess, threading
from pathlib import Path

GROUPS = ["int64", "f32", "f32_64", "f64", "clz"]
# Hand-written members: runtime/compiler-rt/config/6502/members.txt lists the assembly files that replace the generic C of the symbols they export
# (f64 mul and div call __mul32x32, no C group defines it; the f32 add, sub, mul, div and conversions are 4-8x faster and smaller as asm).
MANIFEST = "config/6502/members.txt"
def asm_members(rt):
    return [l.strip() for l in (rt / MANIFEST).read_text().splitlines() if l.strip() and not l.lstrip().startswith("#")]
# Members that replace a generic file's routines at 6502: config.h defines SF_EXT_MULDIV, so lib/fp/f32/{mul,div}.c compile to nothing and
# this file defines __mulsf3 and __divsf3 (the host-built index below sees the generic files' symbols, so these are indexed over them).
CONFIG_MEMBERS = ["config/6502/sf32_6502.c"]
_LOCK = threading.Lock()
_INDEX = {}
_MEMBERS = {}

def index(rt):
    """{symbol: source file relative to rt} of the globals each member defines."""
    with _LOCK:
        if rt not in _INDEX:
            out = subprocess.run(["sh", str(rt / "tests" / "rt_index.sh"), *GROUPS], capture_output=True, text=True)
            if out.returncode:
                raise RuntimeError("rt_index.sh failed: " + out.stderr[-300:])
            _INDEX[rt] = {}
            for line in out.stdout.splitlines():
                symbol, file, _ = (line.split("\t") + ["", ""])[:3]
                _INDEX[rt].setdefault(symbol, file)
            for file in CONFIG_MEMBERS:
                built = subprocess.run(["sh", "-c", 'T=$(mktemp -d); trap "rm -r $T" EXIT; cd "$1" && cc -c -O2 -fno-builtin -Iinclude -DSF_INTABI -DSF_EXT_MULDIV=1 "$2" -o $T/m.o && nm --defined-only -g $T/m.o', "sh", str(rt), file], capture_output=True, text=True)
                if built.returncode:
                    raise RuntimeError(f"{file}: " + built.stderr[-300:])
                for line in built.stdout.splitlines():
                    _INDEX[rt][line.split()[-1]] = file
            for file in asm_members(rt):  # later files win over earlier ones and over the C index
                for line in (rt / file).read_text().splitlines():
                    if line.startswith(".export"):
                        for symbol in line[len(".export"):].split(";")[0].split(","):
                            _INDEX[rt][symbol.strip()] = file
        return _INDEX[rt]

def member(rt, cc, file, flags, target="6502-sim65"):
    """ca65 text of `file` compiled for the 6502 with `flags` (the program's level), cached. Exports are cut to the globals the index names:
    llrm-cc exports statics too, and two members would then define the same helper."""
    if file in asm_members(rt):
        # ca65 runs in the work directory: an .include is pasted in
        return re.sub(r'^\s*\.include\s+"([^"]+)"', lambda m: (rt / file).parent.joinpath(m.group(1)).read_text(), (rt / file).read_text(), flags=re.M)
    key = (rt, cc, file, tuple(flags), target)
    with _LOCK:
        if key in _MEMBERS:
            return _MEMBERS[key]
    keep = set(index(rt))
    done = subprocess.run([cc, f"--target={target}", *flags, "-S", "-w", "-DSF_INTABI", "-include", str(rt / "config/6502/config.h"), "-I", str(rt / "include"), str(rt / file), "-o", "/dev/stdout"],
                          capture_output=True, text=True)
    if done.returncode:
        raise RuntimeError(f"{file}: " + done.stderr[-300:])
    def exports(m):
        names = [n.strip() for n in m.group(1).split(",") if n.strip() in keep]
        return ".export " + ", ".join(names) if names else ""
    text = re.sub(r"^\.export (.*)$", exports, done.stdout, flags=re.M)
    with _LOCK:
        _MEMBERS[key] = text
    return text
