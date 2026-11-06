#!/usr/bin/env python3
"""The compiler-rt members a 6502 program links, built from runtime/compiler-rt as a linker would pick them from an archive: one object per
source file, pulled by an undefined symbol (`tests/rt_index.sh` says which file defines it). Static helpers stay inside their file's object.
Groups (tests/fp_sources.sh): int64 (one routine per file), f32, f32_64, f64. int32.c is not linked: the compiler calls the 32-bit helpers
in mos_rt.s with their own convention."""
import re, subprocess, threading
from pathlib import Path

GROUPS = ["int64", "f32", "f32_64", "f64"]
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
        return _INDEX[rt]

def member(rt, cc, file, flags):
    """ca65 text of `file` compiled for the 6502 with `flags` (the program's level), cached. Exports are cut to the globals the index names:
    llrm-cc exports statics too, and two members would then define the same helper."""
    key = (rt, cc, file, tuple(flags))
    with _LOCK:
        if key in _MEMBERS:
            return _MEMBERS[key]
    keep = set(index(rt))
    done = subprocess.run([cc, "--target=6502-sim65", *flags, "-S", "-w", "-DSF_INTABI", "-include", str(rt / "config/6502/config.h"), "-I", str(rt / "include"), str(rt / file), "-o", "/dev/stdout"],
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
