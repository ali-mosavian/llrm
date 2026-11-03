#!/usr/bin/env python3
"""Join the LLRM_LDV_DUMP rows of an llrm-cc -g compile: LDV (allocator) against VT (valuetrack).

Probe every variable at every note anchor (the slot of LDVNOTE n, the byte offset of VTNOTE n);
classify agree / conflict / ldv-only / vt-only. Usage: ldv_join.py [LDV|VLD] DUMP.log [...]  (VLD: after the block join)
"""
import re
import sys
from collections import Counter, defaultdict


def where(text):
    """Normalize a place: ('R', name), ('C', n), ('E', name) or None for Undef."""
    text = text.strip()
    if text == "Undef":
        return None
    m = re.fullmatch(r"Place\(Register\((\w+)\)\)", text)
    if m:
        return ("R", m.group(1))
    m = re.fullmatch(r"Place\(Cell\((-?\d+)\)\)", text)
    if m:
        return ("C", int(m.group(1)))
    m = re.fullmatch(r"Constant\((-?\d+)\)", text)
    if m:
        return ("K", int(m.group(1)))
    m = re.fullmatch(r"Entry\((\w+)\)", text)
    if m:
        return ("E", m.group(1))
    return ("?", text)


def vt_places(text):
    tokens = re.findall(r"Place\(Register\(\w+\)\)|Place\(Cell[^\]]*?\}?\)\)|Constant\(-?\d+\)|Entry\(\w+\)", text)
    return {where(token) for token in tokens} or {("?", text)}


def main(paths, kind="LDV", quiet=False):
    out = Counter()
    detail = defaultdict(list)
    for path in paths:
        ldv_note, vt_note = {}, {}
        ldv, vt = defaultdict(list), defaultdict(list)
        # (The probes below are per file: a function name repeats across files.)
        for line in open(path, errors="replace"):
            f = line.rstrip("\n").split("\t")
            if f[0] == "LDVNOTE":
                ldv_note.setdefault((f[1], int(f[2])), int(f[3]))
            elif f[0] == "VTNOTE":
                vt_note.setdefault((f[1], int(f[2])), int(f[3]))
            elif f[0] == kind:
                ldv[(f[1], f[2], f[3])].append((int(f[4]), int(f[5]), where(f[6])))
            elif f[0] == "VT":
                vt[(f[1], f[2], f[3])].append((int(f[4]), int(f[5]), vt_places(f[6])))
        for (fn, var, piece) in set(ldv) | set(vt):
            for (nfn, n), slot in ldv_note.items():
                if nfn != fn or (nfn, n) not in vt_note:
                    continue
                off = vt_note[(nfn, n)]
                a = next((w for s, e, w in ldv.get((fn, var, piece), []) if s <= slot < e), None)
                b = next((w for s, e, w in vt.get((fn, var, piece), []) if s <= off < e), None)
                if a is None and b is None:
                    continue
                if a is not None and b is not None:
                    kind = "agree" if a in b else "conflict"
                elif a is not None:
                    kind = "ldv-only"
                else:
                    kind = "vt-only"
                out[kind] += 1
                if kind != "agree":
                    detail[kind].append(f"{path} {fn} var {var} note {n}: ldv={a} vt={b}")
    if not quiet:
        print(dict(out))
        for name, rows in detail.items():
            print(f"-- {name} ({len(rows)})")
            for row in sorted(set(rows))[:12]:
                print("  ", row)
    return out


if __name__ == "__main__":
    args = sys.argv[1:]
    kind = "LDV"
    if args and args[0] in ("LDV", "VLD"):
        kind, args = args[0], args[1:]
    if len(args) > 1:
        total = Counter()
        for path in args:
            total += main([path], kind, quiet=True)
        print(dict(total))
    else:
        main(args, kind)
