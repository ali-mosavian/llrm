#!/usr/bin/env python3
"""Per routine: SDCC (its own function + library) vs ours (jp kernel + the routine's closure), bytes and T-states
for the same C function, args and driver. T = run with the call minus run without it, as the bench does."""
import re, subprocess, sys, tempfile, os
RT = sys.argv[1]
W = tempfile.mkdtemp(prefix="cmp-")
def sh(cmd, inp=None): return subprocess.run(cmd, cwd=W, input=inp, capture_output=True, text=True, timeout=120)
BASE = ["-mz80", "--no-std-crt0", "--code-loc", "0x100", "--data-loc", "0x8000"]
CRT0 = "\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n\t.area _INITIALIZED\n\t.area _GSINIT\n\t.area _GSFINAL\n\t.area _HOME\n"
open(f"{W}/crt0.s", "w").write(CRT0)
assert sh(["sdasz80", "-o", "crt0.rel", "crt0.s"]).returncode == 0
open(f"{W}/rt.s", "w").write(open(RT).read())
assert sh(["sdasz80", "-l", "-o", "rt.rel", "rt.s"]).returncode == 0

# name, C type, [(ptype, arg)], C body, our routine (its closure is read from the runtime source)
U8, U16, S8, S16, U32 = "unsigned char", "unsigned", "signed char", "int", "unsigned long"
ROWS = [
 ("mul8",   U8,  [(U8,"7"),(U8,"6")],        "a*b", "__mulqi3"),
 ("mul16",  U16, [(U16,"1234"),(U16,"56")],  "a*b", "__mulhi3"),
 ("mul16b", U16, [(U16,"56"),(U16,"1234")],  "a*b", "__mulhi3"),
 ("udiv8",  U8,  [(U8,"200"),(U8,"7")],      "a/b", "__udivqi3"),
 ("umod8",  U8,  [(U8,"200"),(U8,"7")],      "a%b", "__umodqi3"),
 ("sdiv8",  S8,  [(S8,"-100"),(S8,"7")],     "a/b", "__divqi3"),
 ("smod8",  S8,  [(S8,"-100"),(S8,"7")],     "a%b", "__modqi3"),
 ("udiv16", U16, [(U16,"40000"),(U16,"10")], "a/b", "__udivhi3"),
 ("umod16", U16, [(U16,"40000"),(U16,"10")], "a%b", "__umodhi3"),
 ("sdiv16", S16, [(S16,"-30000"),(S16,"10")],"a/b", "__divhi3"),
 ("smod16", S16, [(S16,"-30000"),(S16,"10")],"a%b", "__modhi3"),
]
def closure_of(text, routine):
    """The labels a routine reaches: through call/jp/jr to a global label, and by falling through into the next label."""
    order, refs, falls, last = [], {}, {}, None
    for line in text.splitlines():
        line = line.split(";")[0].rstrip()
        m = re.match(r"^([A-Za-z_]\w*):", line)
        if m:
            if last and last[1]: falls[last[0]] = m.group(1)
            order.append(m.group(1)); refs[m.group(1)] = set(); cur = m.group(1); line = line[m.end():]
            last = (cur, True)
        ins = line.split()
        if not ins or ins[0].startswith("."): continue
        if ins[0] in ("call", "jp", "jr") and len(ins) > 1:
            tgt = ins[-1].split(",")[-1]
            if re.match(r"[A-Za-z_]\w*$", tgt): refs[last[0]].add(tgt)
        last = (last[0], not (ins[0] in ("ret", "jp", "jr") and len(ins) == (1 if ins[0] == "ret" else 2)))
    seen, todo = [], [routine]
    while todo:
        n = todo.pop()
        if n in seen: continue
        seen.append(n); todo += list(refs.get(n, ())) + ([falls[n]] if n in falls else [])
    return seen
def code_size_rel(rel):
    return sum(int(m.group(2), 16) for m in (re.match(r"A (\S+) size ([0-9A-Fa-f]+) flags", l) for l in open(rel)) if m and m.group(1) in ("_CODE","_HOME","_GSINIT","_GSFINAL"))
def run(ihx):
    out = sh(["ucsim_z80","-t","Z80","-q","-w",ihx,"-c","-"],"run\nstate\ndump rom 0x8f00 0x8f01\nquit\n").stdout
    assert "Halted" in out, out[-300:]
    clks = int(re.search(r"Total time since last reset=.*\((\d+) clks\)", out).group(1))
    m = re.search(r"0x8f00\s+([0-9a-f]{2})\s+([0-9a-f]{2})", out)
    return clks, int(m.group(1),16) | int(m.group(2),16)<<8
def symbols(mapf):
    syms = {}
    for l in open(mapf):
        m = re.match(r"\s+([0-9A-F]{8})\s+(\S+)\s", l)
        if m and not re.match(r"[sl]__", m.group(2)): syms[m.group(2)] = int(m.group(1), 16)
    return syms
ver = sh(["sdcc","--version"]).stdout.strip().splitlines()[0]
print(ver, "|", sh(["ucsim_z80","-v"]).stdout.splitlines()[0])
print(f"{'row':8} {'SDCC B':>7} {'SDCC T':>7} {'ours B':>7} {'ours T':>7}   verdict")
for name, rt, args, body, routine in ROWS:
    protos = ",".join(f"{t} {'ab'[i]}" for i,(t,_) in enumerate(args))
    kernel_c = f"{rt} f({protos}) {{ return {body}; }}\n"
    call = f"f({', '.join(a for _,a in args)})"
    drv = f"extern {rt} f({','.join(t for t,_ in args)});\n__at(0x8f00) volatile {rt} r;\nvoid main(void) {{\n#ifndef EMPTY\n  r = {call};\n#endif\n}}\n"
    open(f"{W}/d.c","w").write(drv); open(f"{W}/k.c","w").write(kernel_c)
    sh(["sdcc",*BASE,"-c","-DEMPTY","d.c","-o","e.rel"]); sh(["sdcc",*BASE,"-c","d.c","-o","d.rel"])
    assert sh(["sdcc",*BASE,"crt0.rel","e.rel","-o","e.ihx"]).returncode==0
    base_t,_ = run(f"{W}/e.ihx")
    def measure(parts, tag, extra):
        p = sh(["sdcc",*BASE,"crt0.rel","d.rel",*parts,*extra,"-o",f"{tag}.ihx"]); assert p.returncode==0, p.stderr[-300:]
        t,val = run(f"{W}/{tag}.ihx"); return t-base_t, val, f"{W}/{tag}.map"
    # SDCC
    sh(["sdcc",*BASE,"--opt-code-size","-c","k.c","-o","k.rel"])
    st, sval, smap = measure(["k.rel"], "s", [])
    sbytes = None
    for l in open(smap):
        m = re.match(r"(\S+)\s+[0-9A-F]{8}\s+([0-9A-F]{8}) =", l)
    codes = 0
    for l in open(smap):
        m = re.match(r"(\S+)\s+[0-9A-F]{8}\s+([0-9A-F]{8}) =", l)
        if m and m.group(1) in ("_CODE","_HOME","_GSINIT","_GSFINAL"): codes += int(m.group(2),16)
    sbytes = codes - code_size_rel(f"{W}/crt0.rel") - code_size_rel(f"{W}/d.rel")
    # ours: f = jp routine
    open(f"{W}/ok.s","w").write(f"\t.area _CODE\n\t.globl _f, {routine}\n_f:\n\tjp {routine}\n")
    assert sh(["sdasz80","-o","ok.rel","ok.s"]).returncode==0
    ot, oval, omap = measure(["ok.rel","rt.rel"], "o", [])
    syms = symbols(omap)
    names = sorted(syms.items(), key=lambda kv: kv[1])
    def size(label):
        a = syms[label]; nxt = min((v for _,v in names if v > a), default=None)
        return nxt - a
    sizes = {l: size(l) for l in closure_of(open(RT).read(), routine)}
    obytes = 3 + sum(sizes.values())
    mask = 0xFFFF if rt in (U16,S16) else 0xFF
    assert sval & mask == oval & mask, (name, sval, oval)
    verdict = ("bytes " + ("WIN" if obytes < sbytes else "tie" if obytes == sbytes else "LOSS")) + ", T " + ("WIN" if ot < st else "tie" if ot == st else "LOSS")
    print(f"{name:8} {sbytes:7} {st:7} {obytes:7} {ot:7}   {verdict}")
