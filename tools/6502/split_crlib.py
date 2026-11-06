#!/usr/bin/env python3
"""Split crlib modules that hold several routines into one object per routine, as -ffunction-sections + --gc-sections would link them.
usage: split_crlib.py SRC_DIR DST_DIR. A function is a CODE block led by a label; an exported one is a routine, the others are helpers
the routines call and are exported from an object of their own. `s___x_N` cells go with routine x. Modules that are not a plain list of
functions are copied unchanged."""
import re, sys
from pathlib import Path

def split(text):
    """{unit name: source} for a module of several functions, or None when it is not a plain list of them. A function is a CODE block
    (`.segment "CODE"` to the next one) led by a label; a block that is not exported is a helper the routines call (`sf_clz32`) and is
    exported from its own object so the link pulls it once."""
    lines = text.splitlines()
    hdr = [l for l in lines if re.match(r"rc\d+ = ", l)]
    units, bss, seg, cur = [], {}, "CODE", None
    for l in lines:
        if re.match(r"rc\d+ = ", l):
            continue
        if l.startswith(".segment"):
            seg = l.split('"')[1]
            if seg == "CODE":
                cur = None
            continue
        if seg == "BSS":
            m = re.match(r"s___(\w+?)_\d+:", l)
            if not m:
                return None
            bss.setdefault("__" + m.group(1), []).append(l)
            continue
        if cur is None:
            if not l.strip():
                continue
            cur = []
            units.append(cur)
        cur.append(l)
    out = {}
    for u in units:
        exp = [m.group(1) for l in u if (m := re.match(r"\.export (\w+)$", l))]
        label = next((m.group(1) for l in u if (m := re.match(r"(\w+):$", l))), None)
        name = exp[0] if exp else label
        if name is None:
            return None
        head = [] if exp else [f".export {name}"]
        out[name] = "\n".join(hdr + ['.segment "BSS"'] + bss.pop(name, []) + ['.segment "CODE"'] + head + u) + "\n"
    if bss or sum(1 for n in out if n.startswith("__")) < 2:
        return None
    return out

def materialise(src, dst):
    """DST holds SRC's modules, those of several routines split one object per routine; returns DST."""
    dst.mkdir(parents=True, exist_ok=True)
    for p in sorted(src.glob("*.s")):
        parts = split(p.read_text())
        if parts is None:
            (dst / p.name).write_text(p.read_text())
        else:
            for name, body in parts.items():
                (dst / f"{p.stem}__{name.lstrip('_')}.s").write_text(body)
    return dst

if __name__ == "__main__":
    print(materialise(Path(sys.argv[1]), Path(sys.argv[2])))
