"""code_bytes(obj): bytes of every Mach-O section holding instructions (S_ATTR_PURE_INSTRUCTIONS or
S_ATTR_SOME_INSTRUCTIONS), not only __TEXT,__text. text_only(obj): __TEXT,__text alone (the old instrument)."""
import subprocess
def sections(o):
    t = subprocess.run(["otool", "-l", o], capture_output=True, text=True).stdout
    out = []; cur = None
    for l in t.splitlines():
        l = l.strip()
        if l.startswith("sectname"): cur = {"sect": l.split()[1]}
        elif l.startswith("segname") and cur is not None and "seg" not in cur: cur["seg"] = l.split()[1]
        elif l.startswith("size") and cur is not None and "size" not in cur: cur["size"] = int(l.split()[1], 16)
        elif l.startswith("flags") and cur is not None and "flags" not in cur:
            cur["flags"] = int(l.split()[1], 16); out.append(cur); cur = None
    return out
def is_code(s): return bool(s["flags"] & 0x80000000 or s["flags"] & 0x400)
def code_bytes(o): return sum(s["size"] for s in sections(o) if is_code(s))
def text_only(o): return sum(s["size"] for s in sections(o) if s["seg"] == "__TEXT" and s["sect"] == "__text")

import re
def funcs(o):
    """Instruction count per symbol, from `otool -tv`."""
    t = subprocess.run(["otool", "-tv", o], capture_output=True, text=True).stdout; cur = None; cnt = {}
    for l in t.splitlines():
        m = re.match(r"^(\S+):$", l)
        if m: cur = m[1]; cnt[cur] = 0; continue
        if cur and re.match(r"^[0-9a-f]{16}\t", l): cnt[cur] += 1
    return cnt
def kernel(o):
    """(bytes, insns) of the kernel: every code section minus the harness's orig_main and report."""
    f = funcs(o); skip = ("_orig_main", "_report")
    return code_bytes(o) - 4 * sum(f.get(s, 0) for s in skip), sum(v for k, v in f.items() if k not in skip)
