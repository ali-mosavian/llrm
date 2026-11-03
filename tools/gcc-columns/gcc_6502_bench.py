#!/usr/bin/env python3
"""gcc mos6502 column: the C kernels of kernels.c on the 6502 bench's method.

Method (crates/target/llrm-6502/tests/bench.rs): an asm driver sets the
arguments, calls the kernel 1 and 101 times in sim65 (cl65 -t sim6502), and exits
0 when the result equals the hand-derived value. clocks = (T101 - T1) / 100 - 9
(the loop's dec cnt + bne), so a call includes argument set-up and return.
bytes = the CODE segment of the kernel's object alone (od65 --dump-segsize);
helper routines it calls (mul, div) are noted separately.
"""
import argparse, hashlib, os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
B, W = "B", "W"
BUF = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3]
# (name, args, wide result, expected); P(sym, off) is a pointer argument.
def P(sym, off): return ("P", sym, off)
K = [
    ("zero", [], False, 0),
    ("add8", [(B, 200), (B, 100)], False, 44),
    ("add16", [(W, 300), (W, 5000)], True, 5300),
    ("max8", [(B, 3), (B, 9)], False, 9),
    ("max16", [(W, 1000), (W, 300)], True, 1000),
    ("sum_arr", [(B, 10)], False, 39),
    ("find", [(B, 9)], False, 5),
    ("fill", [(B, 8)], False, 7),
    ("popcount", [(B, 0xB5)], False, 5),
    ("mul8", [(B, 7), (B, 6)], False, 42),
    ("fib", [(B, 12)], True, 144),
    ("gcd", [(B, 48), (B, 18)], False, 6),
    ("sum4", [], False, 9),
    ("slen", [P("str", 0)], True, 11),
    ("scmp", [P("str", 0), P("str", 6)], True, 0xFFF1),
    ("mcopy", [P("buf", 8), P("buf", 0), (B, 8)], False, 6),
    ("crc16", [P("buf", 0), (B, 12)], True, 0xA166),
    ("mul16", [(W, 1234), (W, 56)], True, 3568),
    ("digits", [(W, 40000)], False, 5),
    ("reverse", [P("buf", 0), (B, 8)], False, 6),
    ("bsearch8", [P("tab", 0), (B, 8), (B, 17)], False, 5),
    ("sum16", [P("buf", 0), (B, 16)], True, 80),
    ("schr", [P("str", 0), (B, 111)], False, 4),
    ("lz4", [P("lz", 0), P("lzout", 0), (W, 634)], True, 634),
    ("lz4c", [P("lz", 0), P("lzout", 0), (W, 634)], True, 52785),
    ("xs8", [(B, 0xA5), (B, 5)], False, 107),
    ("minmax", [P("buf", 0), (B, 16)], False, 8),
    ("fletcher", [P("buf", 0), (B, 16)], True, 15952),
    ("hash16", [P("str", 0)], True, 51393),
    ("dotx", [P("buf", 0), P("tab", 0), (B, 8)], False, 101),
]
ZP = 26
CC65_STARTUP = {"callmain", "condes", "crt0", "exehdr", "paravirt", "pushax", "zerobss", "zeropage"}


def load_hand(d):
    """tests/bench/hand specs: name, kernel, args, wide, rel, expect."""
    out = []
    for f in sorted(os.listdir(d)):
        if not f.endswith(".spec"):
            continue
        kv = dict(l.split(": ", 1) for l in open(os.path.join(d, f)).read().splitlines() if ": " in l)
        args = []
        for m in re.finditer(r"([BWP])\(([^)]*)\)", kv["args"]):
            if m.group(1) == "P":
                sym, off = m.group(2).split(",")
                args.append(P(sym.strip(), int(off)))
            else:
                args.append((m.group(1), int(m.group(2))))
        rel = kv["rel"].strip()
        out.append((kv["kernel"], args, kv["wide"].strip() == "1", int(kv["expect"]),
                    os.path.join(d, f[:-5] + ".c"), None if rel == "-" else rel))
    return out


def sh(cmd, cwd, **kw):
    return subprocess.run(["bash", "-c", "ulimit -v 16000000; timeout 120 \"$@\"", "_"] + cmd, cwd=cwd,
                          capture_output=True, text=True, **kw)


def md5(p):
    return hashlib.md5(open(p, "rb").read()).hexdigest()


def level_flags(lvl, noivopts_os):
    """-fno-ivopts only at -Os: at -O1/-O2 ivopts keeps the pointer walks that are cheaper."""
    return [f"-{lvl}"] + (["-fno-ivopts"] if noivopts_os and lvl == "Os" else [])


def cc1_line(path):
    """Table header line: the md5 only; a local path in a table is refused by the landing runner."""
    return f"cc1 md5 {md5(path)}"


def driver(args, wide, expect, calls, rel=None):
    s = [".export _main", ".export _buf, _str, _tab, _lz, _lzout, _mix, _mix2, _num, _pat, _set, _dst"]
    s += [f".exportzp rc{i}" for i in range(ZP)]
    s.append(".segment \"ZEROPAGE\"")
    s += [f"rc{i}: .res 1" for i in range(ZP)]
    s += [".segment \"CODE\"", ".import _KERNEL", "_main:",
          "  lda #0\n  sta rc0\n  lda #$80\n  sta rc1", f"  lda #{calls}\n  sta cnt", "loop:"]
    slot = 0
    for a in args:
        size = 1 if a[0] == B else 2
        if size == 2 and slot & 1:
            slot += 1
        reg = 2 + slot
        if a[0] == "P":
            lo, hi = f"#<(_{a[1]}+{a[2]})", f"#>(_{a[1]}+{a[2]})"
        else:
            lo, hi = f"#{a[1] & 255}", f"#{a[1] >> 8}"
        s.append(f"  lda {lo}\n  sta rc{reg}")
        if size == 2:
            s.append(f"  lda {hi}\n  sta rc{reg + 1}")
        slot += size
    s.append("  jsr _KERNEL\n  dec cnt\n  bne loop")
    want = expect if wide else expect & 255
    if rel:
        s.append(f"  lda rc2\n  cmp #<(_{rel}+{want})\n  bne bad")
        s.append(f"  lda rc3\n  cmp #>(_{rel}+{want})\n  bne bad")
    else:
        s.append(f"  lda rc2\n  cmp #{want & 255}\n  bne bad")
        if wide:
            s.append(f"  lda rc3\n  cmp #{want >> 8}\n  bne bad")
    s.append("  lda #0\n  jmp $FFF9\nbad:\n  lda #1\n  jmp $FFF9")
    s += [".segment \"DATA\"", "_buf: .byte " + ",".join(map(str, BUF)),
          "_str: .byte \"hello world\",0", "_tab: .byte 2,5,8,11,14,17,20,23",
          "_mix: .byte \"Hello, World 42!\",0", "_mix2: .byte \"hELLO, wORLD 42?\",0",
          "_num: .byte \"  -4829xyz\",0", "_pat: .byte \"o w\",0", "_set: .byte \"lo\",0",
          "_lz: .byte " + open(os.path.join(HERE, "lz.txt")).read().strip(),
          ".segment \"BSS\"", "cnt: .res 1", "_lzout: .res 640", "_dst: .res 64"]
    return "\n".join(s) + "\n"


SIM = "sim65"
TARGET = "sim6502"
COMPILER = "gcc"
CFLAGS = []


def sim(binpath, cwd):
    """(exit code, cycles, instructions or None): the simulator's own counts."""
    p = sh([SIM, "-c", "-x", "200000000", binpath], cwd)
    m = re.search(r"(\d+) cycles", p.stdout)
    if not m:
        raise RuntimeError("no cycle count: " + (p.stdout + p.stderr)[-200:])
    i = re.search(r"(\d+) instructions", p.stdout)
    return p.returncode, int(m.group(1)), int(i.group(1)) if i else None


HAND_HELPERS = {"mulqi3", "mulhi3", "udivhi3", "umodhi3", "divhi3", "modhi3", "memset", "memcpy", "memmove"}


def data_size(obj, cwd):
    p = sh(["od65", "--dump-segsize", obj], cwd)
    return sum(int(m.group(2)) for m in re.finditer(r"(RODATA|DATA):\s*(\d+)", p.stdout))


def code_size(obj, cwd):
    """CODE, RODATA and DATA: constant tables the compiler makes are part of the kernel."""
    p = sh(["od65", "--dump-segsize", obj], cwd)
    return sum(int(m.group(2)) for m in re.finditer(r"(CODE|RODATA|DATA):\s*(\d+)", p.stdout))


def helper_closure(asm, exports, imports):
    """Helper modules the kernel's asm calls, and the ones they call."""
    todo = [exports[m] for m in re.findall(r"\b(?:jsr|jmp)\s+(\w+)", asm) if m in exports]
    seen = []
    while todo:
        h = todo.pop()
        if h not in seen:
            seen.append(h)
            todo += [exports[i] for i in imports[h] if i in exports]
    return seen


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cc1", required=True)
    ap.add_argument("--kernels", default="")
    ap.add_argument("--levels", default="O1,O2,Os")
    ap.add_argument("--rt", default=os.path.join(HERE, "rt-6502"))
    ap.add_argument("--src", default=os.path.join(HERE, "..", "..", "crates/target/llrm-6502/tests/bench/kernels.c"))
    ap.add_argument("--out", default="")
    ap.add_argument("--hand", default=os.path.join(HERE, "..", "..", "crates/target/llrm-6502/tests/bench/hand"), help="dir of hand-corpus .spec/.c files, '' to skip")
    ap.add_argument("--sim", default="sim65", help="sim65, or a build that also prints '<n> instructions'")
    ap.add_argument("--cpu", default="6502", choices=["6502", "65c02"])
    ap.add_argument("--tsv", default="")
    ap.add_argument("--tsv-header", default="")
    ap.add_argument("--noivopts-os", action="store_true", help="column gcc-noivopts-Os: -fno-ivopts at -Os")
    a = ap.parse_args()
    global SIM, TARGET, COMPILER, CFLAGS
    if a.cpu == "65c02":
        TARGET, COMPILER, CFLAGS = "sim65c02", "gcc-65c02", ["-m65c02"]
    if a.noivopts_os:
        COMPILER += "-noivopts-Os"
    SIM = os.path.abspath(a.sim) if os.sep in a.sim else a.sim
    kernels = [(n, ar, wd, ex, None, None) for n, ar, wd, ex in K]
    if a.hand:
        kernels += load_hand(a.hand)
    want = [k for k in a.kernels.split(",") if k] or [k[0] for k in kernels]
    w = tempfile.mkdtemp(prefix="gcc6502-")
    src = os.path.abspath(a.src)
    # helper library: ld65 pulls a member only when something references it
    objs = []
    for f in sorted(os.listdir(a.rt)):
        if f.endswith(".s"):
            r = sh(["ca65", "-t", TARGET, "-o", f[:-2] + ".o", os.path.join(os.path.abspath(a.rt), f)], w)
            assert r.returncode == 0, r.stderr
            objs.append(f[:-2] + ".o")
    sh(["rm", "-f", "rt.lib"], w)
    r = sh(["ar65", "a", "rt.lib"] + objs, w)
    assert r.returncode == 0, r.stderr
    helper_bytes = {o[:-2]: code_size(o, w) for o in objs}
    exports, imports = {}, {}
    for o in objs:
        text = open(os.path.join(os.path.abspath(a.rt), o[:-2] + ".s")).read()
        for sym in re.findall(r"\.export\s+(\w+)", text):
            exports[sym] = o[:-2]
        imports[o[:-2]] = re.findall(r"\.import\s+(\w+)", text)
    open(f"{w}/empty.c", "w").write("")
    r = sh([a.cc1, "-version", "-quiet", "empty.c", "-o", "/dev/null"], w)
    ver = next(l for l in (r.stdout + r.stderr).splitlines() if "version" in l)
    hdr = [cc1_line(a.cc1), f"version: {ver}",
           "assembler/linker/sim: " + sh(["sim65", "--version"], w).stderr.strip().splitlines()[0] + " (ca65/ld65 via cl65 -t " + TARGET + ")",
           "flags: cc1 -quiet -<level>" + (" (+ -fno-ivopts at -Os)" if a.noivopts_os else "") + " -fno-tree-loop-distribute-patterns -DK_<name> kernels.c; clocks=(T101-T1)/100-9; bytes=CODE+RODATA+DATA of the kernel object plus the helpers it calls plus the caller's result move"]
    rows, bad, notes = {}, [], {}
    for name, args, wide, expect, ksrc, rel in kernels:
        if name not in want:
            continue
        for lvl in a.levels.split(","):
            cell = (name, lvl)
            p = sh([a.cc1, "-quiet"] + level_flags(lvl, a.noivopts_os) + CFLAGS + ([] if ksrc else [f"-DK_{name}"]) + [ksrc or src, "-o", "k.s"], w)
            if p.returncode:
                bad.append((*cell, "compile: " + (p.stderr or p.stdout)[-200:].replace("\n", " "))); continue
            asm = open(f"{w}/k.s").read()
            kernel_sym = "_" + name
            ts = []
            ins_ = []
            fail = None
            for calls in (1, 101):
                open(f"{w}/d.s", "w").write(driver(args, wide, expect, calls, rel).replace("_KERNEL", kernel_sym))
                p = sh(["cl65", "-t", TARGET, "-m", "x.map", "-o", "x.bin", "d.s", "k.s", "rt.lib"], w)
                if p.returncode:
                    fail = "link: " + (p.stderr or p.stdout)[-200:].replace("\n", " "); break
                # a symbol gcc calls must not resolve to cc65's library: other ABI
                alien = sorted(set(re.findall(r"sim65(?:c)?02\.lib\((\w+)\.o\)", open(f"{w}/x.map").read())) - CC65_STARTUP)
                if alien:
                    fail = "links the cc65 library (" + ",".join(alien) + "): other ABI"; break
                try:
                    rc, t, ni = sim("x.bin", w)
                except RuntimeError as e:
                    fail = str(e); break
                if rc != 0:
                    fail = f"WRONG result (exit {rc}), hand-derived {expect:#x}"; break
                ts.append(t)
                ins_.append(ni)
            if fail:
                bad.append((*cell, fail)); continue
            r = sh(["ca65", "-t", TARGET, "-o", "k.o", "k.s"], w)
            body = code_size("k.o", w)
            used = helper_closure(asm, exports, imports)
            extra = sum(helper_bytes[h] for h in used)
            # the loop's dec cnt + bne are 2 instructions
            n_ins = None
            if None not in ins_ and (ins_[1] - ins_[0]) % 100 == 0:
                n_ins = (ins_[1] - ins_[0]) // 100 - 2
            # Result convention: ours/llvm-mos/cc65 return in A (A:X), gcc in rc2 (rc2:rc3), so the
            # caller's lda rc2 (+ ldx rc3) is charged to gcc: 2 B / 3 clk / 1 insn, wide 4 B / 6 clk / 2 insn.
            mv_b, mv_c, mv_i = (4, 6, 2) if wide else (2, 3, 1)
            rows[cell] = (body + extra + mv_b, (ts[1] - ts[0]) // 100 - 9 + mv_c, None if n_ins is None else n_ins + mv_i)
            note = f"incl. result move +{mv_b} B +{mv_c} clk +{mv_i} insn"
            if used:
                note += f"; body {body} B + " + ", ".join(f"__{h} {helper_bytes[h]} B" for h in used)
                if any(h in HAND_HELPERS for h in used):
                    note = "hand helper, not codegen; " + note
            if data_size(f"{w}/k.o", w) >= 64:
                note = "table, not codegen; " + note
            notes[cell] = note
            if (ts[1] - ts[0]) % 100:
                bad.append((*cell, f"cycles per call not whole: {(ts[1]-ts[0])/100}")); del rows[cell]; continue
    lv = a.levels.split(",")
    out = ["# gcc mos6502 spike column", ""] + hdr + ["", "| kernel | " + " | ".join(f"{l} B/clk" for l in lv) + " |", "|---|" + "---|" * len(lv)]
    for name, *_ in kernels:
        if name in want:
            cells = [rows.get((name, l)) for l in lv]
            out.append(f"| {name} | " + " | ".join("%d/%d" % c[:2] if c else "n/a" for c in cells) + " |")
    if bad:
        out += ["", "## n/a causes"] + [f"- {n} {l}: {m}" for n, l, m in bad]
    text = "\n".join(out) + "\n"
    if a.out:
        open(a.out, "w").write(text)
    if a.tsv:
        nb = {(n, l): m for n, l, m in bad}
        t = [a.tsv_header, "target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote"]
        for name, *_ in kernels:
            for l in lv:
                if name in want:
                    c = rows.get((name, l))
                    t.append("\t".join(["6502", name, l, COMPILER] + ([str(c[0]), str(c[1]), "" if c[2] is None else str(c[2]), notes.get((name, l), "")] if c else ["", "", "", "n/a: " + nb.get((name, l), "not run")])))
        open(a.tsv, "w").write("\n".join(t) + "\n")
    print(text)


if __name__ == "__main__":
    main()
