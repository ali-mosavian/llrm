#!/usr/bin/env python3
"""Z80 baseline: the 6502 bench kernels (tests/bench/kernels.c) compiled by SDCC and run on ucsim.

Per kernel and SDCC variant: bytes (linked code incl. library routines, minus crt0 and driver),
T-states and executed instructions (run with the call minus run without it, so argument set-up,
call, return and the result store count, as in the 6502 bench). A row counts only if r equals the
value worked out by hand.
"""
import os, re, subprocess, sys, tempfile, shutil

HERE = os.path.dirname(os.path.abspath(__file__))
KERNELS_C = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.join(HERE, "kernels.c")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "RESULTS-sdcc.md")

U8, U16 = "unsigned char", "unsigned"
BUF, STR, TAB = 0x9000, 0x9010, 0x9020
R = 0x8F00

# name, return type, [(param type, argument expression)], expected
K = [
    ("zero", U8, [], 0), ("add8", U8, [(U8, "200"), (U8, "100")], 44),
    ("add16", U16, [(U16, "300"), (U16, "5000")], 5300), ("max8", U8, [(U8, "3"), (U8, "9")], 9),
    ("max16", U16, [(U16, "1000"), (U16, "300")], 1000), ("sum_arr", U8, [(U8, "10")], 39),
    ("find", U8, [(U8, "9")], 5), ("fill", U8, [(U8, "8")], 7), ("popcount", U8, [(U8, "0xB5")], 5),
    ("mul8", U8, [(U8, "7"), (U8, "6")], 42), ("fib", U16, [(U8, "12")], 144),
    ("gcd", U8, [(U8, "48"), (U8, "18")], 6), ("sum4", U8, [], 9),
    ("slen", U16, [("const char*", "str")], 11),
    ("scmp", "int", [("const char*", "str"), ("const char*", "str+6")], 0xFFF1),
    ("mcopy", U8, [(U8 + "*", "buf+8"), ("const " + U8 + "*", "buf"), (U8, "8")], 6),
    ("crc16", U16, [("const " + U8 + "*", "buf"), (U8, "12")], 0xA166),
    ("mul16", U16, [(U16, "1234"), (U16, "56")], 3568), ("digits", U8, [(U16, "40000")], 5),
    ("reverse", U8, [(U8 + "*", "buf"), (U8, "8")], 6),
    ("bsearch8", U8, [("const " + U8 + "*", "tab"), (U8, "8"), (U8, "17")], 5),
    ("sum16", U16, [("const " + U8 + "*", "buf"), (U8, "16")], 80),
    ("schr", U8, [("const char*", "str"), (U8, "111")], 4),
]

VARIANTS = {
    "size": ["--opt-code-size"],
    "size+ma": ["--opt-code-size", "--max-allocs-per-node", "100000"],
    "speed": ["--opt-code-speed"],
    "speed+ma": ["--opt-code-speed", "--max-allocs-per-node", "100000"],
}
BASE = ["-mz80", "--no-std-crt0", "--code-loc", "0x100", "--data-loc", "0x8000"]

CRT0 = """\t.module crt0
\t.globl _main
\t.area _HEADER (ABS)
\t.org 0
\tld sp,#0
\tcall _main
\thalt
\t.area _CODE
\t.area _DATA
\t.area _INITIALIZED
\t.area _GSINIT
\t.area _GSFINAL
\t.area _HOME
"""

DATA = """\t.module data
\t.area _DABS (ABS)
\t.org 0x9000
\t.globl _buf, _str, _tab
_buf:\t.db 3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3
_str:\t.ascii "hello world"
\t.db 0,0,0,0,0
_tab:\t.db 2,5,8,11,14,17,20,23
"""

CODE_AREAS = {"_CODE", "_HOME", "_GSINIT", "_GSFINAL", "_INITIALIZER"}


def sh(cmd, cwd, inp=None):
    p = subprocess.run(cmd, cwd=cwd, input=inp, capture_output=True, text=True, timeout=120)
    return p


def code_size(rel):
    n = 0
    for line in open(rel):
        m = re.match(r"A (\S+) size ([0-9A-Fa-f]+) flags", line)
        if m and m.group(1) in CODE_AREAS:
            n += int(m.group(2), 16)
    return n


def linked_code(mapf):
    n = 0
    for line in open(mapf):
        m = re.match(r"(\S+)\s+[0-9A-F]{8}\s+([0-9A-F]{8}) =", line)
        if m and m.group(1) in CODE_AREAS:
            n += int(m.group(2), 16)
    return n


def run(ihx, cwd, dump):
    cmds = "run\nstate\n" + "".join(f"dump rom 0x{a:x} 0x{a+1:x}\n" for a in dump) + "quit\n"
    p = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", ihx, "-c", "-"], cwd, cmds)
    out = p.stdout
    if "Halted" not in out:
        raise RuntimeError("did not halt:\n" + out[-600:])
    clks = int(re.search(r"Total time since last reset=.*\((\d+) clks\)", out).group(1))
    inst = int(re.search(r"Inst=\s*(\d+)", out).group(1))
    vals = []
    for a in dump:
        m = re.search(rf"0x{a:04x}\s+([0-9a-f]{{2}})\s+([0-9a-f]{{2}})", out)
        vals.append(int(m.group(1), 16) | int(m.group(2), 16) << 8)
    return clks, inst, vals[0]


def driver(name, ret, args):
    protos = ", ".join(t for t, _ in args) or "void"
    pre = "extern unsigned char buf[]; extern char str[]; extern unsigned char tab[];\n"
    pre += f"extern {ret} {name}({protos});\n__at(0x{R:x}) volatile {ret} r;\n"
    call = f"r = {name}({', '.join(a for _, a in args)});"
    return pre + "void main(void) {\n#ifndef EMPTY\n  " + call + "\n#endif\n}\n"


def main():
    w = tempfile.mkdtemp(prefix="z80bench-")
    open(f"{w}/crt0.s", "w").write(CRT0)
    open(f"{w}/data.s", "w").write(DATA)
    for s in ("crt0", "data"):
        assert sh(["sdasz80", "-o", f"{s}.rel", f"{s}.s"], w).returncode == 0
    ver = sh(["sdcc", "--version"], w).stdout.strip()
    uv = sh(["ucsim_z80", "-v"], w).stdout.strip().splitlines()[0]
    rows, bad = {}, []
    for name, ret, args, want in K:
        open(f"{w}/d.c", "w").write(driver(name, ret, args))
        sh(["sdcc", *BASE, "-c", "-DEMPTY", "d.c", "-o", "e.rel"], w)
        sh(["sdcc", *BASE, "-c", "d.c", "-o", "d.rel"], w)
        # empty run: driver without the call
        p = sh(["sdcc", *BASE, "crt0.rel", "data.rel", "e.rel", "-o", "e.ihx"], w)
        base_t, base_i, _ = run("e.ihx", w, [R])
        base_code = code_size(f"{w}/crt0.rel") + code_size(f"{w}/e.rel")
        for vn, vf in VARIANTS.items():
            kc = f"-DK_{name}"
            p = sh(["sdcc", *BASE, *vf, "-c", kc, KERNELS_C, "-o", "k.rel"], w)
            if p.returncode:
                bad.append((name, vn, "compile: " + p.stderr[-300:])); continue
            p = sh(["sdcc", *BASE, *vf, "crt0.rel", "data.rel", "d.rel", "k.rel", "-o", "x.ihx"], w)
            if p.returncode:
                bad.append((name, vn, "link: " + p.stderr[-300:])); continue
            t, i, got = run("x.ihx", w, [R])
            mask = 0xFFFF if ret != U8 else 0xFF
            if got & mask != want & mask:
                bad.append((name, vn, f"result {got & mask:#x}, hand-derived {want & mask:#x}")); continue
            bytes_ = linked_code(f"{w}/x.map") - code_size(f"{w}/crt0.rel") - code_size(f"{w}/d.rel")
            rows[(name, vn)] = (bytes_, t - base_t, i - base_i)
    # Instrument guard: rows counted by hand from SDCC's asm and the Zilog T-state table.
    # zero = call 17 + xor 4 + ret 10 + ld (nn),a 13; max8(3,9) = 7+7+17+4+4+4+12+4+10+13.
    assert rows[("zero", "size")] == (2, 44, 4), rows[("zero", "size")]
    assert rows[("max8", "size")][1:] == (82, 10), rows[("max8", "size")]
    with open(OUT, "w") as f:
        f.write(f"# SDCC on the 6502 bench kernels (Z80)\n\n{ver}\n{uv} (Z80), sz80\n\n")
        f.write("Cost = run with the call minus run without it (argument set-up, call, return and the result store included). "
                "Bytes = linked code incl. library routines. T-states from ucsim `clks`; instructions from its `Inst` "
                "(exact without breakpoints, checked on 10 NOPs + HALT).\n\n")
        f.write("| kernel | " + " | ".join(f"{v} B / T / insn" for v in VARIANTS) + " |\n|---|" + "---|" * len(VARIANTS) + "\n")
        for name, *_ in K:
            cells = []
            for vn in VARIANTS:
                r = rows.get((name, vn))
                cells.append("%d / %d / %d" % r if r else "FAIL")
            f.write(f"| {name} | " + " | ".join(cells) + " |\n")
        if bad:
            f.write("\n## Failures\n" + "".join(f"- {n} {v}: {m}\n" for n, v, m in bad))
    print(open(OUT).read())
    shutil.rmtree(w)


main()
