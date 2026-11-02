#!/usr/bin/env python3
"""gcc column for the Z80 kernels: cc1 (spike port) -> sdasz80 -> sdldz80 -> ucsim_z80.

    gcc_z80_bench.py --cc1 PATH [--kernels a,b] [--levels O1,O2,Os] [--rt DIR] [--out FILE]

Same corpus, driver shape and cost definition as crates/target/llrm-z80/bench/bench_sdcc.py: cost = run with the
call minus run without it; bytes = linked _CODE minus crt0 and driver, so library helpers called are counted. A cell
counts only if the result equals the value worked out by hand; otherwise it is n/a with the reason.
"""
import argparse, hashlib, os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
U8, U16 = "unsigned char", "unsigned"
BUF, STR, TAB, R = 0x9000, 0x9010, 0x9020, 0x8F00
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
    ("sum16", U16, [("const " + U8 + "*", "buf"), (U8, "16")], 80), ("schr", U8, [("const char*", "str"), (U8, "111")], 4),
]
CRT0 = "\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n"
DATA = f"""\t.module data
\t.area _DABS (ABS)
\t.org 0x{BUF:x}
\t.globl _buf, _str, _tab
_buf:\t.db 3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3
_str:\t.ascii "hello world"
\t.db 0,0,0,0,0
_tab:\t.db 2,5,8,11,14,17,20,23
"""


def sh(cmd, cwd, inp=None):
    return subprocess.run(["bash", "-c", "ulimit -v 16000000; exec timeout 120 \"$@\"", "x"] + cmd, cwd=cwd,
                          input=inp, capture_output=True, text=True)


def md5(path):
    return hashlib.md5(open(path, "rb").read()).hexdigest()


def code_size(rel):
    return sum(int(m.group(2), 16) for m in (re.match(r"A (\S+) size ([0-9A-Fa-f]+) flags", l) for l in open(rel))
               if m and m.group(1) in ("_CODE", "_HOME", "_GSINIT", "_GSFINAL", "_INITIALIZER"))


def linked_code(mapf):
    return sum(int(m.group(2), 16) for m in (re.match(r"(\S+)\s+[0-9A-F]{8}\s+([0-9A-F]{8}) =", l) for l in open(mapf))
               if m and m.group(1) in ("_CODE", "_HOME", "_GSINIT", "_GSFINAL", "_INITIALIZER"))


def run(ihx, cwd):
    cmds = f"run\nstate\ndump rom 0x{R:x} 0x{R+1:x}\nquit\n"
    out = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", ihx, "-c", "-"], cwd, cmds).stdout
    if "Halted" not in out:
        raise RuntimeError("did not halt")
    clks = int(re.search(r"Total time since last reset=.*\((\d+) clks\)", out).group(1))
    inst = int(re.search(r"Inst=\s*(\d+)", out).group(1))
    m = re.search(rf"0x{R:04x}\s+([0-9a-f]{{2}})\s+([0-9a-f]{{2}})", out)
    return clks, inst, int(m.group(1), 16) | int(m.group(2), 16) << 8


def driver(name, ret, args):
    protos = ", ".join(t for t, _ in args) or "void"
    s = "extern unsigned char buf[]; extern char str[]; extern unsigned char tab[];\n"
    s += f"extern {ret} {name}({protos});\n#define RES (*(volatile {ret} *)0x{R:x})\n"
    s += "void main(void) {\n#ifndef EMPTY\n  RES = " + f"{name}({', '.join(a for _, a in args)});\n#endif\n}}\n"
    return s


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cc1", required=True)
    ap.add_argument("--kernels", default="")
    ap.add_argument("--levels", default="O1,O2,Os")
    ap.add_argument("--rt", default=os.path.join(HERE, "rt-z80"), help="dir of libgcc helper .s files, one routine each")
    ap.add_argument("--src", default=os.path.join(HERE, "..", "..", "crates/target/llrm-6502/tests/bench/kernels.c"))
    ap.add_argument("--out", default="")
    ap.add_argument("--tsv", default="", help="matrix TSV rows (target prog level compiler bytes clocks instrs note)")
    ap.add_argument("--tsv-header", default="")
    a = ap.parse_args()
    want = [k for k in a.kernels.split(",") if k] or [k[0] for k in K]
    w = tempfile.mkdtemp(prefix="gccz80-")
    open(f"{w}/crt0.s", "w").write(CRT0)
    open(f"{w}/data.s", "w").write(DATA)
    for s in ("crt0", "data"):
        assert sh(["sdasz80", "-o", f"{s}.rel", f"{s}.s"], w).returncode == 0
    rts = []
    if os.path.isdir(a.rt):
        for f in sorted(os.listdir(a.rt)):
            if f.endswith(".s"):
                assert sh(["sdasz80", "-o", f[:-2] + ".rel", os.path.join(a.rt, f)], w).returncode == 0, f
                rts.append(f[:-2] + ".rel")
    # selective library: sdldz80 pulls members of a text list only when referenced
    open(f"{w}/rt.lib", "w").write("".join(f"{r}\n" for r in rts))
    open(f"{w}/empty.c", "w").write("")
    r = sh([a.cc1, "-version", "-quiet", "empty.c", "-o", "/dev/null"], w)
    ver = next(l for l in (r.stdout + r.stderr).splitlines() if "version" in l)
    hdr = [f"cc1: {a.cc1} md5 {md5(a.cc1)}", f"version: {ver}",
           "assembler/linker: sdasz80/sdldz80 (SDCC 4.5.0), ucsim_z80",
           "flags: cc1 -quiet -<level> -DK_<name> kernels.c; driver cc1 -quiet -O1"]
    base_flags = ["-quiet"]
    rows, bad = {}, []
    for name, ret, args, want_v in K:
        if name not in want:
            continue
        open(f"{w}/d.c", "w").write(driver(name, ret, args))
        for tag, defs in (("e", ["-DEMPTY"]), ("d", [])):
            p = sh([a.cc1, *base_flags, "-O1", *defs, "d.c", "-o", f"{tag}.s"], w)
            assert p.returncode == 0, p.stderr[-400:]
            assert sh(["sdasz80", "-o", f"{tag}.rel", f"{tag}.s"], w).returncode == 0
        def link(objs, out):
            lk = ["-i", "-m", "-b", "_HEADER=0", "-b", "_CODE=0x100", "-b", "_DATA=0x8000", out]
            return sh(["sdldz80", *lk, "-k", w, "-l", "rt", *objs] if rts else ["sdldz80", *lk, *objs], w)
        # sdldz80 takes: options, output name, then object files
        p = link(["crt0.rel", "data.rel", "e.rel"], "e")
        if p.returncode:
            raise RuntimeError("baseline link failed: " + p.stderr[-300:])
        bt, bi, _ = run("e.ihx", w)
        for lvl in a.levels.split(","):
            cell = (name, lvl)
            p = sh([a.cc1, *base_flags, f"-{lvl}", f"-DK_{name}", a.src, "-o", "k.s"], w)
            if p.returncode:
                bad.append((*cell, "compile: " + (p.stderr or p.stdout)[-200:].replace("\n", " "))); continue
            if sh(["sdasz80", "-o", "k.rel", "k.s"], w).returncode:
                bad.append((*cell, "assemble")); continue
            p = link(["crt0.rel", "data.rel", "d.rel", "k.rel"], "x")
            if p.returncode:
                bad.append((*cell, "link: " + p.stderr[-200:].replace("\n", " "))); continue
            try:
                t, i, got = run("x.ihx", w)
            except RuntimeError as e:
                bad.append((*cell, str(e))); continue
            mask = 0xFFFF if ret != U8 else 0xFF
            if got & mask != want_v & mask:
                bad.append((*cell, f"WRONG result {got & mask:#x}, hand-derived {want_v & mask:#x}")); continue
            b = linked_code(f"{w}/x.map") - code_size(f"{w}/crt0.rel") - code_size(f"{w}/d.rel")
            rows[cell] = (b, t - bt, i - bi)
    lv = a.levels.split(",")
    out = ["# gcc z80 spike column", ""] + hdr + ["", "| kernel | " + " | ".join(f"{l} B/T/insn" for l in lv) + " | best B | best T |", "|---|" + "---|" * (len(lv) + 2)]
    for name, *_ in K:
        if name not in want:
            continue
        cells = [rows.get((name, l)) for l in lv]
        ok = [c for c in cells if c]
        out.append(f"| {name} | " + " | ".join("%d/%d/%d" % c if c else "n/a" for c in cells) + " | "
                   + (str(min(c[0] for c in ok)) if ok else "n/a") + " | " + (str(min(c[1] for c in ok)) if ok else "n/a") + " |")
    if bad:
        out += ["", "## n/a causes"] + [f"- {n} {l}: {m}" for n, l, m in bad]
    if a.tsv:
        notes = {(n, l): m for n, l, m in bad}
        t = [a.tsv_header, "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote"]
        for name, *_ in K:
            for l in lv:
                if name in want:
                    c = rows.get((name, l))
                    t.append("\t".join(["z80", name, l, "gcc"] + ([str(x) for x in c] + [""] if c else ["", "", "", "n/a: " + notes.get((name, l), "not run")])))
        open(a.tsv, "w").write("\n".join(t) + "\n")
    text = "\n".join(out) + "\n"
    if a.out:
        open(a.out, "w").write(text)
    print(text)


main()
