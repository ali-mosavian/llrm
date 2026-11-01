#!/usr/bin/env python3
"""Check runtime/z80_rt.s against C semantics on the oracle simulator.

    check.py RUNTIME.s

Each routine is assembled with sdasz80, linked at 0x6000 with a harness that calls it over a table of inputs, and run on
ucsim_z80. Byte routines are checked on every input pair; word and long routines on edge values plus seeded random ones.
Exit 1 on any difference. Prints the T-states of a few calls (call and ret included).
"""
import re, subprocess, sys, os, random, tempfile
RT = sys.argv[1]
W = tempfile.mkdtemp(prefix="rt-")

def sh(cmd, inp=None):
    return subprocess.run(cmd, cwd=W, input=inp, capture_output=True, text=True, timeout=300)

def build(src_name, text):
    open(f"{W}/{src_name}.s", "w").write(text)
    p = sh(["sdasz80", "-l", "-o", f"{src_name}.rel", f"{src_name}.s"]); assert p.returncode == 0, p.stdout + p.stderr
    return f"{src_name}.rel"

RTREL = None
def link(main_rel, name):
    p = sh(["sdldz80", "-i", "-m", "-b", "_CODE=0x6000", f"{name}.ihx", main_rel, RTREL]); assert p.returncode == 0, p.stdout + p.stderr
    return f"{name}.ihx"

def run(ihx, dump=None):
    cmds = "run\nstate\n" + (f"dump rom 0x{dump[0]:x} 0x{dump[1]:x}\n" if dump else "") + "quit\n"
    p = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", ihx, "-c", "-"], cmds)
    out = p.stdout
    assert "Halted" in out, out[-500:]
    clks = int(re.search(r"\((\d+) clks\)", out).group(1))
    data = None
    if dump:
        lo, hi = dump
        data = {}
        for line in out.splitlines():
            m = re.match(r"0x([0-9a-f]{4})\s+(.*)", line)
            if not m: continue
            base = int(m.group(1), 16) & ~7
            want = [a for a in range(max(lo, base), min(hi, base + 7) + 1)]
            toks = m.group(2).split()
            for a, t in zip(want, toks):
                data[a] = int(t, 16)
    return clks, data

# signature: how the harness loads inputs and stores the result.
SIGS = {
  "A,L>A":  dict(isz=2, osz=1, load="\tld\ta,0(ix)\n\tld\tl,1(ix)\n", store="\tld\t(hl),a\n"),
  "HL,DE>DE": dict(isz=4, osz=2, load="\tld\tl,0(ix)\n\tld\th,1(ix)\n\tld\te,2(ix)\n\tld\td,3(ix)\n", store="\tld\t(hl),e\n\tinc\thl\n\tld\t(hl),d\n"),
  "L,A>L": dict(isz=5, osz=4, load="\tld\tl,0(ix)\n\tld\th,1(ix)\n\tld\te,2(ix)\n\tld\td,3(ix)\n\tld\ta,4(ix)\n", pre="\tld\t(0x8010),de\n\tld\t(0x8012),hl\n", store="\tld\thl,#0x8010\n\tld\tde,(0x8000)\n\tld\tbc,#4\n\tldir\n\tdec\tde\n\tex\tde,hl\n"),
  "HL,A>DE": dict(isz=3, osz=2, load="\tld\tl,0(ix)\n\tld\th,1(ix)\n\tld\ta,2(ix)\n", store="\tld\t(hl),e\n\tinc\thl\n\tld\t(hl),d\n"),
}

def check(name, sig, ref, vectors, label=""):
    s = SIGS[sig]
    ok = True
    n_total = len(vectors)
    CH = 3000
    for start in range(0, n_total, CH):
        vs = vectors[start:start + CH]
        tbl = "".join("\t.db " + ",".join(str(b) for b in enc(sig, v)) + "\n" for v in vs)
        # results pointer in memory at 0x8000; entries counter at 0x8002
        text = f"""\t.area _HEADER (ABS)
\t.org 0
\tld sp,#0
\tld ix,#table
\tld hl,#0x9000
\tld (0x8000),hl
\tld hl,#{len(vs)}
\tld (0x8002),hl
next:
{s['load']}\tcall {name}
{s.get('pre', '')}\tld hl,(0x8000)
{s['store']}\tinc hl
\tld (0x8000),hl
\tld bc,#{s['isz']}
\tadd ix,bc
\tld hl,(0x8002)
\tdec hl
\tld (0x8002),hl
\tld a,h
\tor l
\tjr nz,next
\thalt
\t.area _CODE
\t.globl {name}
table:
{tbl}"""
        # table must not overlap the runtime: link table above 0x2000 via .org in a separate area
        text = text.replace("\t.area _CODE\n\t.globl "+name+"\ntable:", "\t.org 0x1000\n\t.globl "+name+"\ntable:")
        rel = build("h", text)
        ihx = link(rel, "h")
        _, data = run(ihx, (0x9000, 0x9000 + len(vs) * s['osz'] - 1))
        for i, v in enumerate(vs):
            got = int.from_bytes(bytes(data[0x9000 + i * s['osz'] + j] for j in range(s['osz'])), "little")
            want = ref(*decode(sig, v))
            if got != want:
                print(f"FAIL {name}{label} {decode(sig, v)}: got {got}, want {want}"); ok = False
                if sum(1 for _ in range(1)) and i > 3: break
        if not ok: break
    return ok

def enc(sig, v):
    if sig == "L,A>L": return (v[0] & 255, v[0] >> 8 & 255, v[0] >> 16 & 255, v[0] >> 24, v[1])
    if sig == "A,L>A": return v
    if sig == "HL,DE>DE": return (v[0] & 255, v[0] >> 8, v[1] & 255, v[1] >> 8)
    return (v[0] & 255, v[0] >> 8, v[1])

def decode(sig, v):
    return v

def cost(name, sig, args):
    """T-states of one call incl. its call+ret: run with the call minus without."""
    regs = {"A,L>A": "\tld a,#%d\n\tld l,#%d\n", "HL,DE>DE": "\tld hl,#%d\n\tld de,#%d\n", "HL,A>DE": "\tld hl,#%d\n\tld a,#%d\n", "L,A>L": "\tld hl,#%d\n\tld de,#0\n\tld a,#%d\n"}[sig] % args
    base = f"\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n{regs}"
    t = {}
    for tag, call in (("with", f"\tcall {name}\n"), ("without", "")):
        rel = build("c", base + call + "\thalt\n\t.area _CODE\n\t.globl "+name+"\n")
        ihx = link(rel, "c")
        t[tag], _ = run(ihx)
    return t["with"] - t["without"]

def rtsize(name):
    # bytes of a routine: distance to the next global label in the listing
    lst = open(f"{W}/rt.lst").read() if os.path.exists(f"{W}/rt.lst") else ""
    return None

def main():
    global RTREL
    p = sh(["cp", RT, "rt.s"]); 
    RTREL = build("rt", open(f"{W}/rt.s").read())
    rnd = random.Random(1)
    edges16 = [0, 1, 2, 3, 7, 8, 9, 10, 15, 16, 100, 255, 256, 257, 1000, 1234, 4096, 32767, 32768, 32769, 40000, 65534, 65535]
    allok = True
    # byte multiply, exhaustive
    pairs = [(a, b) for a in range(256) for b in range(256)]
    allok &= check("__mulqi3", "A,L>A", lambda a, b: a * b & 255, pairs)
    allok &= check("__udivqi3", "A,L>A", lambda a, b: a // b if b else 255, [p for p in pairs if p[1]])
    allok &= check("__umodqi3", "A,L>A", lambda a, b: a % b, [p for p in pairs if p[1]])
    for nm, f in (("__ashlqi3", lambda a, b: a << b & 255), ("__lshrqi3", lambda a, b: a >> b), ("__ashrqi3", lambda a, b: (a - 256 if a > 127 else a) >> b & 255)):
        allok &= check(nm, "A,L>A", f, [(a, b) for a in range(256) for b in range(8)])
    w = [(rnd.choice(edges16) if rnd.random() < .3 else rnd.getrandbits(rnd.choice([4, 8, 12, 16])), rnd.choice(edges16) if rnd.random() < .3 else rnd.getrandbits(rnd.choice([4, 8, 12, 16]))) for _ in range(6000)] + [(a, b) for a in edges16 for b in edges16]
    allok &= check("__mulhi3", "HL,DE>DE", lambda a, b: a * b & 0xFFFF, w)
    wd = [p for p in w if p[1]]
    allok &= check("__udivhi3", "HL,DE>DE", lambda a, b: a // b, wd)
    allok &= check("__umodhi3", "HL,DE>DE", lambda a, b: a % b, wd)
    for nm, f in (("__ashlhi3", lambda a, b: a << b & 0xFFFF), ("__lshrhi3", lambda a, b: a >> b), ("__ashrhi3", lambda a, b: (a - 65536 if a > 32767 else a) >> b & 0xFFFF)):
        allok &= check(nm, "HL,A>DE", f, [(a, b) for a in edges16 + [rnd.getrandbits(16) for _ in range(200)] for b in range(16)])
    def s8(x): return x - 256 if x > 127 else x
    def s16(x): return x - 65536 if x > 32767 else x
    def tdiv(a, b): q = abs(a) // abs(b); return q if (a < 0) == (b < 0) else -q
    sp = [p for p in pairs if p[1]]
    allok &= check("__divqi3", "A,L>A", lambda a, b: tdiv(s8(a), s8(b)) & 255, sp)
    allok &= check("__modqi3", "A,L>A", lambda a, b: (s8(a) - tdiv(s8(a), s8(b)) * s8(b)) & 255, sp)
    allok &= check("__divhi3", "HL,DE>DE", lambda a, b: tdiv(s16(a), s16(b)) & 0xFFFF, wd)
    allok &= check("__modhi3", "HL,DE>DE", lambda a, b: (s16(a) - tdiv(s16(a), s16(b)) * s16(b)) & 0xFFFF, wd)
    l32 = [0, 1, 255, 256, 65535, 65536, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF, 0x12345678] + [rnd.getrandbits(32) for _ in range(60)]
    s32 = lambda x: x - (1 << 32) if x >> 31 else x
    for nm, f in (("__ashlsi3", lambda a, b: a << b & 0xFFFFFFFF), ("__lshrsi3", lambda a, b: a >> b), ("__ashrsi3", lambda a, b: s32(a) >> b & 0xFFFFFFFF)):
        allok &= check(nm, "L,A>L", f, [(a, b) for a in l32 for b in range(32)])
    print("ALL OK" if allok else "FAILURES")
    for nm, sig, args in (("__mulqi3", "A,L>A", (7, 6)), ("__mulqi3", "A,L>A", (200, 255)), ("__udivqi3", "A,L>A", (200, 7)), ("__mulhi3", "HL,DE>DE", (1234, 56)), ("__mulhi3", "HL,DE>DE", (56, 1234)), ("__udivhi3", "HL,DE>DE", (40000, 10)), ("__umodhi3", "HL,DE>DE", (40000, 10)), ("__ashlhi3", "HL,A>DE", (1, 5)), ("__divqi3", "A,L>A", (200, 7)), ("__divhi3", "HL,DE>DE", (40000, 10)), ("__ashlsi3", "L,A>L", (1, 20))):
        print(f"{nm}{args}: {cost(nm, sig, args)} T")
    sys.exit(0 if allok else 1)
main()
