#!/usr/bin/env python3
"""Refuse a z80 matrix cell whose two sides compile with different codegen flags.

sdcc_mismatch(ours, variant) and gcc_mismatch(ours, gcc): `ours` is the clang flag list of the cell (the corpus CLANG_FLAGS
plus the program's `-f` flags), the other side its compiler flags. Plumbing is dropped (-S -c -emit-llvm -o -I -D -Xclang
pairs, the input). A flag on one side only must be listed with its reason: anything else is refused, as arm64 lost 24 of 72
cells to -fomit-frame-pointer on one side (#8708 wasm_flag_check.py)."""
import sys

PLUMBING = {"-S", "-c", "-emit-llvm", "-w"}
PAIRS = {"-o", "-I", "-target", "-Xclang"}
# Ours-only, SDCC has no counterpart or the same behaviour by construction.
OURS_ONLY_SDCC = {
    "-fpack-struct=1": "SDCC lays structs out unpadded",
    "-fno-vectorize": "SDCC has no vectorizer", "-fno-slp-vectorize": "SDCC has no vectorizer",
    "-fomit-frame-pointer": "SDCC's frame choice is its own", "-fno-stack-protector": "SDCC has none",
    "-fno-stack-clash-protection": "SDCC has none", "-fno-asynchronous-unwind-tables": "SDCC has none",
    "-U_FORTIFY_SOURCE": "no libc headers on either side",
    "-fno-inline-functions": "SDCC has no inliner", "-fno-inline-functions-called-once": "SDCC has no inliner",
    "-fwrapv": "SDCC wraps signed overflow", "-fexcess-precision=standard": "SDCC float has no excess precision",
}
# The four variants of the owner table: the one list the harness scripts and the table's check read.
SDCC_VARIANTS = {
    "sdcc-size": ["--fsigned-char", "--opt-code-size"],
    "sdcc-size-ma": ["--fsigned-char", "--opt-code-size", "--max-allocs-per-node", "200000"],
    "sdcc-speed": ["--fsigned-char", "--opt-code-speed"],
    "sdcc-speed-ma": ["--fsigned-char", "--opt-code-speed", "--max-allocs-per-node", "200000"],
}
# A SDCC variant is exactly these: a char signedness, one optimisation goal, optionally the allocator budget.
SDCC_GOALS = {"--opt-code-size", "--opt-code-speed"}
# gcc-only, no counterpart on ours.
GCC_ONLY = {"-fno-tree-loop-distribute-patterns": "gcc would call memset; ours is priced with its runtime member linked",
            "-w": "warnings", "-std=gnu17": "clang's default"}


def clang_flags(tokens):
    """(triple, set of codegen flags, optimisation level) of a clang flag list."""
    triple, flags, level, i = None, set(), None, 0
    while i < len(tokens):
        t = tokens[i]
        if t == "-target":
            i += 1
            triple = tokens[i]
        elif t in PAIRS:
            i += 1
        elif t.startswith("-O") and len(t) <= 3:
            level = t[2:]
        elif t in PLUMBING or t.startswith(("-I", "-D", "-o")) or t.endswith((".c", ".i")):
            pass
        else:
            flags.add(t)
        i += 1
    return triple, flags, level


def sdcc_mismatch(ours, variant):
    """Empty when the sides agree, else a one-line reason."""
    triple, flags, _ = clang_flags(ours)
    if triple not in (None, "msp430"):
        return f"triple {triple}: the corpus IR is made for msp430"
    sdcc = set(variant)
    goals = sdcc & SDCC_GOALS
    if len(goals) != 1:
        return f"SDCC variant has goals {sorted(goals)}"
    extra = sdcc - SDCC_GOALS - {"--fsigned-char", "--max-allocs-per-node", "200000"}
    if extra:
        return f"SDCC-only flags {sorted(extra)}"
    if ("-fsigned-char" in flags) != ("--fsigned-char" in sdcc):
        return "char signedness differs"
    if ("--max-allocs-per-node" in sdcc) != ("200000" in sdcc):
        return "--max-allocs-per-node without its value"
    unknown = sorted(f for f in flags - {"-fsigned-char"} if f not in OURS_ONLY_SDCC)
    if unknown:
        return f"ours-only flags with no SDCC counterpart listed: {unknown}"
    return ""


def cell_mismatch(source, level, variant, run_py=None):
    """The check for one owner-table cell: `source` the program's text, `level` 1 s z 2 3, `variant` a table column name.
    Ours is the corpus CLANG_FLAGS plus the program's -f flags (tests/programs/run.py); an unknown variant is refused."""
    if variant not in SDCC_VARIANTS:
        return f"unknown SDCC variant {variant}"
    import importlib.util
    from pathlib import Path
    path = run_py or Path(__file__).resolve().parent.parent / "crates/target/llrm-z80/tests/programs/run.py"
    spec = importlib.util.spec_from_file_location("z80_run", path)
    run = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(run)
    class Text:
        def read_text(self):
            return source
    return sdcc_mismatch(["-O" + level, *run.CLANG_FLAGS, *run.program_flags(Text())[0]], SDCC_VARIANTS[variant])


def gcc_mismatch(ours, gcc):
    """The gcc column: the program's own -f flags go to both sides, the level is the same."""
    _, of, ol = clang_flags(ours)
    _, gf, gl = clang_flags(gcc)
    if ol != gl:
        return f"level differs: ours -O{ol}, gcc -O{gl}"
    shared = {"-fsigned-char", "-fpack-struct=1"}
    program = lambda s: {f for f in s if f not in OURS_ONLY_SDCC or f in ("-fno-inline-functions", "-fno-inline-functions-called-once", "-fwrapv", "-fexcess-precision=standard")} - shared
    only_gcc = {f for f in gf - of if f not in GCC_ONLY and not f.startswith("-D")}
    if (shared - of) or (shared - gf):
        return f"missing {sorted((shared - of) | (shared - gf))}"
    if program(of) != program(gf) - set(GCC_ONLY):
        return f"program flags differ: ours {sorted(program(of))}, gcc {sorted(program(gf))}"
    if only_gcc - program(gf):
        return f"gcc-only flags {sorted(only_gcc - program(gf))}"
    return ""


if __name__ == "__main__":
    why = sdcc_mismatch(sys.argv[1].split(), sys.argv[2].split())
    if why:
        print(f"z80_flag_check: {why}", file=sys.stderr)
        sys.exit(2)
