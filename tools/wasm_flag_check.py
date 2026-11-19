#!/usr/bin/env python3
"""Refuse a wasm32 matrix whose two sides compile with different codegen flags.

usage: wasm_flag_check.py OURS_ARGV_LINE PIN_FLAG...
OURS_ARGV_LINE is the clang command line llrm-cc ran (one string); the rest is the pin's flag list.
Plumbing is not codegen: the triple is compared on its own; -S/-c/-emit-llvm, -o, -I, -D, the input file and the
pass-disable `-Xclang` pairs llrm-cc adds are dropped. Whatever remains must be equal as a set.
(The arm64 matrix lost 24 of 72 cells' agreement to -fomit-frame-pointer on one side only.)"""
import sys

PLUMBING = {"-S", "-c", "-emit-llvm"}
PAIRS = {"-o", "-I", "-target", "-Xclang"}


def canon_triple(t):
    parts = t.split("-")
    return "-".join(parts + ["unknown"] * (3 - len(parts)))


def codegen_flags(tokens):
    """(triple, set of codegen flags) of one clang command line."""
    triple, flags, i = None, set(), 0
    while i < len(tokens):
        t = tokens[i]
        if t.startswith("--target="):
            triple = canon_triple(t.split("=", 1)[1])
        elif t == "-target":
            i += 1
            triple = canon_triple(tokens[i])
        elif t in PAIRS:
            i += 1
        elif t in PLUMBING or t.startswith(("-I", "-D", "-o")) or t.endswith((".c", ".cpp", ".i")):
            pass
        else:
            flags.add(t)
        i += 1
    return triple, flags


def mismatch(ours, pin):
    """Empty when the sides agree, else a one-line reason."""
    (ot, of), (pt, pf) = codegen_flags(ours), codegen_flags(pin)
    if ot != pt:
        return f"triple differs: ours {ot}, pin {pt}"
    if of != pf:
        return f"flags differ: only ours {sorted(of - pf)}, only pin {sorted(pf - of)}"
    return ""


if __name__ == "__main__":
    why = mismatch(sys.argv[1].split(), sys.argv[2:])
    if why:
        print(f"wasm_flag_check: {why}", file=sys.stderr)
        sys.exit(2)
