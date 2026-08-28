#!/usr/bin/env python3
"""
Unit tests for qbe, on bytes built here rather than on a compiled program.

    python3 tools/qbe/unit.py [-v]

Three other things are tested elsewhere and none of them replaces this one.
dectest.py checks the decoder against ndisasm on real programs; regress.sh
checks that whole programs compute the same answers after being rewritten;
price.py says what a rewrite costs. All of those were green while the
classifier reported "no half" for A1, while a pair copy was treated as dead,
and while a negate inherited another instruction's address. Agreement
between implementations is not correctness when both were written from the
same wrong idea, and a program computing the right answer is not evidence
about the piece that happened not to run.

So these are small, exhaustive where they can be, and property-based where
enumeration would not finish.
"""
import sys, os, struct, itertools, subprocess
here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, here)
from qbopt import declen
from qbopt.declen import length, T as TAB, T0F, BAD
from qbopt.lift import (decode1, lift, needed, regions, sizeof, encode, emit_region,
                  LOAD, ALUM, ALUV, STORE, MOVE, NEG, FIXUP, PAIRS, REG, LOREG)

VERBOSE = "-v" in sys.argv
fails, checks, group = [], 0, [""]

def G(name):
    group[0] = name
    if VERBOSE: print(f"\n-- {name}")

def ok(cond, what):
    global checks
    checks += 1
    if not cond: fails.append(f"[{group[0]}] {what}")
    elif VERBOSE: print(f"   ok  {what}")

def eq(got, want, what):
    ok(got == want, f"{what}" + ("" if got == want else f"  (got {got!r}, want {want!r})"))

def hx(s): return bytes.fromhex(s.replace(" ", ""))

ALU = sorted(PAIRS)                      # 23 and, 0B or, 33 xor, 03 add, 2B sub
HI  = {lo: PAIRS[lo][0] for lo in ALU}
BASES = {0x06: 2, 0x46: 1, 0x86: 2}      # ModRM mod/rm -> displacement bytes

# ==========================================================================
G("length decoder: the forms BC and the runtime emit")
for enc, n, what in [
    ("A1 5E 00",          3, "mov ax,moffs16"),
    ("66 A1 5E 00",       4, "mov eax,moffs32 -- 66 does not change moffs"),
    ("8B 16 60 00",       4, "mov dx,[disp16]"),
    ("8B 46 E8",          3, "mov ax,[bp+disp8]"),
    ("8B 86 00 01",       4, "mov ax,[bp+disp16]"),
    ("8B C1",             2, "mov ax,cx -- register direct"),
    ("83 D2 00",          3, "adc dx,imm8"),
    ("81 C2 00 01",       4, "add dx,imm16"),
    ("66 81 C2 00 01 00 00", 7, "add edx,imm32 under 66"),
    ("F7 D8",             2, "neg ax -- F7 /3 takes no immediate"),
    ("F7 06 5E 00 34 12", 6, "test [disp16],imm16 -- F7 /0 does"),
    ("F6 06 5E 00 34",    5, "test [disp16],imm8"),
    ("0F 85 1A 16",       4, "jnz near"),
    ("0F B6 C1",          3, "movzx"),
    ("0F A4 C1 04",       4, "shld r/m,r,imm8"),
    ("66 FF 36 5E 00",    5, "push dword [disp16]"),
    ("9A 0F 00 15 00",    5, "call far ptr16:16"),
    ("EA 0F 00 15 00",    5, "jmp far ptr16:16"),
    ("C2 08 00",          3, "ret imm16"),
    ("C8 04 00 00",       4, "enter imm16,imm8"),
    ("C4 46 E8",          3, "les ax,[bp+disp8]"),
    ("26 8A 07",          3, "a segment prefix is a prefix"),
    ("F3 A4",             2, "rep movsb"),
    ("67 66 8D 04 80",    5, "lea eax,[eax+eax*4] -- 32-bit addressing"),
]:
    eq(length(hx(enc), 0), n, what)

G("length decoder: unknown opcodes give up rather than guess")
unknown = [op for op in range(256) if TAB[op] == BAD]
ok(len(unknown) > 0, "some opcodes are unknown")
for op in unknown[:8]:
    eq(length(bytes([op, 0, 0, 0, 0, 0]), 0), None, f"opcode {op:02X} bails")
eq(length(hx("0F FF"), 0), None, "an unknown two-byte opcode bails")

G("length decoder: every ModRM, exhaustively")
# For each of the two addressing sizes, every mod/rm must give a length that
# matches what the encoding says. A table typo shows up here and nowhere else
# until it corrupts a program.
for mod in range(4):
    for rm in range(8):
        m = (mod << 6) | rm
        body = bytes([0x8B, m]) + b"\x11\x22\x33\x44"
        want = 2
        if mod == 0 and rm == 6: want = 4          # [disp16]
        elif mod == 1: want = 3                    # disp8
        elif mod == 2: want = 4                    # disp16
        eq(length(body, 0), want, f"16-bit addressing mod={mod} rm={rm}")
for mod in range(4):
    for rm in range(8):
        m = (mod << 6) | rm
        body = bytes([0x67, 0x8B, m, 0x24]) + b"\x11\x22\x33\x44"
        want = 3                                    # 67 + opcode + modrm
        if rm == 4 and mod != 3: want += 1          # a sib byte
        if mod == 0 and rm == 5: want += 4          # [disp32]
        elif mod == 0 and rm == 4: want += 0
        elif mod == 1: want += 1
        elif mod == 2: want += 4
        eq(length(body, 0), want, f"32-bit addressing mod={mod} rm={rm}")

G("length decoder: never runs off the end")
for n in range(1, 6):
    for op in (0x8B, 0xA1, 0x81, 0x9A, 0x0F, 0xC8, 0xF7):
        r = length(bytes([op]) * n, 0)
        ok(r is None or r <= 15, f"truncated {op:02X} x{n} gives {r}")

# ==========================================================================
G("classifier: every register, half, operation and addressing form")
for opcode, kind in ((0x8B, 'ld'), (0x89, 'st')):
    for reg in range(4):
        pair, half = REG[reg]
        for base, dl in BASES.items():
            m = base | (reg << 3)
            b = bytes([opcode, m]) + (b"\xE8" if dl == 1 else b"\x5E\x00")
            a = decode1(b, 0)
            ok(a is not None, f"{kind} reg={reg} base={base:02X} decodes")
            if a:
                eq(a[0], kind, f"{kind} reg={reg} base={base:02X} kind")
                eq((a[1], a[3]), (pair, half),
                   f"{kind} reg={reg} base={base:02X} pair/half")
                eq((a[7], a[8]), (base, dl),
                   f"{kind} reg={reg} base={base:02X} form")
for lo in ALU:
    for op in (lo, HI[lo]):
        for reg in range(4):
            pair, half = REG[reg]
            m = 0x06 | (reg << 3)
            a = decode1(bytes([op, m, 0x5E, 0x00]), 0)
            ok(a is not None and a[0] == 'op', f"alu {op:02X} reg={reg}")
            if a: eq(a[4], op, f"alu {op:02X} keeps its opcode")

G("classifier: register to register")
for dreg in range(4):
    for sreg in range(4):
        m = 0xC0 | (dreg << 3) | sreg
        a = decode1(bytes([0x8B, m]), 0)
        same_half = REG[dreg][1] == REG[sreg][1]
        if same_half:
            ok(a is not None and a[0] == 'mv', f"mov r{dreg},r{sreg} is a move")
            if a: eq((a[1], a[2]), (REG[dreg][0], REG[sreg][0]),
                     f"mov r{dreg},r{sreg} pairs")
        else:
            eq(a, None, f"mov r{dreg},r{sreg} crosses halves and is not a pair")

G("classifier: what it must refuse")
eq(decode1(hx("8B 07"), 0), None,     "[bx] is not a form BC uses for a long")
eq(decode1(hx("8B 20"), 0), None,     "reg=sp is not a long register")
eq(decode1(hx("8B 36 5E 00"), 0), None, "reg=si is not a long register")
eq(decode1(hx("87 06 5E 00"), 0), None, "xchg is not one of the operations")
eq(decode1(hx("8B C4"), 0), None,     "mov ax,sp is not a pair move")

# ==========================================================================
G("lift: one value per idiom")
def one(h):
    b = hx(h); return lift(b, 0, len(b))[0]

v = one("A1 5E 00 8B 16 60 00   23 06 5A 00 23 16 5C 00   A3 62 00 89 16 64 00")
eq([x.op for x in v], [LOAD, ALUM, STORE], "load, operate, store")
eq((v[1].s1, v[2].s1), (0, 1), "the chain is wired up")
eq(v[0].mem, 0x5E, "the load keeps the low half's displacement")

v = one("8B 0E 5E 00 8B 1E 60 00   8B D3 8B C1   A3 62 00 89 16 64 00")
eq([x.op for x in v], [LOAD, MOVE, STORE], "a pair copy is a value")
eq((v[1].pair, v[1].src_pair), (0, 1), "and knows both pairs")

v = one("A1 5E 00 8B 16 60 00  8B 0E 62 00 8B 1E 64 00  33 C1 33 D3")
eq([x.op for x in v], [LOAD, LOAD, ALUV], "a cross-pair operation")
eq((v[2].s1, v[2].s2), (0, 1), "reading both values")

v = one("A1 5E 00 8B 16 60 00   F7 D8 83 D2 00 F7 DA")
eq([x.op for x in v], [LOAD, NEG], "neg/adc/neg is one negate")
eq(v[1].end - v[1].at, 7, "consuming seven bytes")

v = one("8B 0E 5E 00 8B 1E 60 00   F7 D9 83 D3 00 F7 DB")
eq([x.op for x in v], [LOAD, NEG], "the negate in cx:bx too")

v = one("A1 5E 00 8B 16 60 00   89 56 EA 89 46 E8")
eq([x.op for x in v], [LOAD, STORE], "a spill, high half written first")
eq((v[1].mem, v[1].at), (-24, 7), "keeps the low displacement and the earlier address")

G("lift: what it must refuse")
eq(one("A1 5E 00 8B 16 62 00"), [],
   "halves four apart are two variables, not one long")
eq(one("A1 5E 00 8B 0E 60 00"), [],
   "halves in different pairs are not a pair")
eq(one("A1 5E 00 8B 16 60 00 03 06 5A 00 03 16 5C 00")[1:], [],
   "add/add is two integers; only add/adc is a long")
eq(one("A1 5E 00 8B 16 60 00 2B 06 5A 00 2B 16 5C 00")[1:], [],
   "sub/sub likewise")
eq(one("23 06 5A 00 23 16 5C 00"), [],
   "an operation with nothing loaded has no value to work from")
eq(one("A3 5E 00 89 16 60 00"), [],
   "a store with nothing loaded likewise")
v = one("A1 5E 00 8B 16 60 00  90  23 06 5A 00 23 16 5C 00")
eq([x.op for x in v], [LOAD], "an opaque instruction invalidates the pairs")

G("lift: two adjacent integers must not look like a long")
# Integer variables are two bytes apart, so a load pair and two independent
# 16-bit loads are the same displacements. Only the register pairing tells
# them apart, which is why the half test is not optional.
eq(one("A1 5E 00 A1 60 00"), [], "two moffs loads into ax are not a pair")
eq(one("8B 06 5E 00 8B 0E 60 00"), [], "ax then cx is not a pair")

# ==========================================================================
G("regions and liveness")
b = hx("A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 00"
       "90 90"
       "A1 62 00 8B 16 64 00 A3 66 00 89 16 68 00")
v, _ = lift(b, 0, len(b))
eq(len(regions(v)), 2, "opaque bytes split a region")
need = needed(v)
eq(need[1], True, "a value in a register when a region ends is live")
eq(all(need[n] for n, x in enumerate(v) if x.op == STORE), True,
   "a store is always needed")

b = hx("A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 00 A3 62 00 89 16 64 00")
v, _ = lift(b, 0, len(b)); need = needed(v)
eq(all(need), True, "everything feeding a store is needed, transitively")

# ==========================================================================
G("sizing and encoding agree, for every form")
# If these disagree the region overruns the code after it or wastes what it
# claimed. Two parallel switch statements do not stay in step on their own.
corpus = ("A1 5E 00 8B 16 60 00  23 06 5A 00 23 16 5C 00  A3 62 00 89 16 64 00"
          "8B 0E 5E 00 8B 1E 60 00  0B 0E 5A 00 0B 1E 5C 00"
          "8B D3 8B C1  33 C1 33 D3"
          "F7 D8 83 D2 00 F7 DA"
          "8B 46 E8 8B 56 EA  03 46 E8 13 56 EA  89 46 E8 89 56 EA"
          "8B 86 00 01 8B 96 02 01")
v, _ = lift(hx(corpus), 0, len(hx(corpus)))
ok(len(v) >= 10, f"the corpus lifted {len(v)} values")
seen = set()
for n, x in enumerate(v):
    eq(sizeof(x, None), len(encode(x)), f"v{n} {x.op} base {x.base:02X}")
    seen.add((x.op, x.base, x.pair))
ok(len({op for op, _, _ in seen}) >= 5, f"covering {len({o for o,_,_ in seen})} value kinds")

G("encoding: exact bytes")
for h, want, what in [
    ("A1 5E 00 8B 16 60 00",            "66a15e00",    "mov eax,[disp16]"),
    ("8B 0E 5E 00 8B 1E 60 00",         "668b0e5e00",  "mov ecx,[disp16]"),
]:
    eq(encode(one(h)[0]).hex(), want, what)
v = one("A1 5E 00 8B 16 60 00  8B 46 E8 8B 56 EA")
eq(encode(v[1]).hex(), "668b46e8", "bp-relative keeps its ModRM and disp8")
v = one("A1 5E 00 8B 16 60 00  23 06 5A 00 23 16 5C 00")
eq(encode(v[1]).hex(), "6623065a00", "and eax,[disp16]")
v = one("8B 0E 5E 00 8B 1E 60 00  23 0E 5A 00 23 1E 5C 00")
eq(encode(v[1]).hex(), "66230e5a00", "and ecx,[disp16]")
v = one("A1 5E 00 8B 16 60 00  8B 0E 62 00 8B 1E 64 00  33 C1 33 D3")
eq(encode(v[2]).hex(), "6633c1", "xor eax,ecx -- mod 11, reg eax, rm ecx")
v = one("A1 5E 00 8B 16 60 00 F7 D8 83 D2 00 F7 DA")
eq(encode(v[1]).hex(), "66f7d8", "neg eax")
v = one("8B 0E 5E 00 8B 1E 60 00 F7 D9 83 D3 00 F7 DB")
eq(encode(v[1]).hex(), "66f7d9", "neg ecx")
eq(FIXUP[0].hex(), "668bd066c1ea10", "pair 0's high half comes back from eax")
eq(FIXUP[1].hex(), "668bd966c1eb10", "pair 1's from ecx")

G("emission: a region never grows, and the slack is jumped over")
for h in ["A1 5E 00 8B 16 60 00 23 06 5A 00 23 16 5C 00 A3 62 00 89 16 64 00",
          "8B 0E 5E 00 8B 1E 60 00 0B 0E 5A 00 0B 1E 5C 00 89 0E 62 00 89 1E 64 00",
          "A1 5E 00 8B 16 60 00 F7 D8 83 D2 00 F7 DA A3 62 00 89 16 64 00"]:
    b = hx(h); v, _ = lift(b, 0, len(b)); need = needed(v)
    for reg in regions(v):
        span = v[reg[-1]].end - v[reg[0]].at
        out = emit_region(v, need, reg)
        if out is None: continue
        eq(len(out), span, f"padded to exactly the {span} bytes it replaced")
        core = sum(sizeof(v[n], None) for n in reg if need[n])
        core += sum(len(FIXUP[p]) for p in {v[n].pair for n in reg if need[n]})
        if span - core >= 2:
            eq(out[core], 0xEB, "the slack begins with a jump")
            eq(out[core+1], span - core - 2, "clearing exactly the slack")
            ok(all(x == 0x90 for x in out[core+2:]), "and the rest is nops")

G("emission: too small a region is refused, not overrun")
b = hx("8B 0E 5E 00 8B 1E 60 00 89 0E 62 00 89 1E 64 00")   # 16 bytes, pair 1
v, _ = lift(b, 0, len(b)); need = needed(v)
for reg in regions(v):
    out = emit_region(v, need, reg)
    span = v[reg[-1]].end - v[reg[0]].at
    ok(out is None or len(out) <= span, "never writes past what it replaced")

# ==========================================================================
G("fuzz: the decoder against ndisasm")
try:
    import random
    random.seed(20260828)
    blob = bytes(random.randrange(256) for _ in range(4000))
    r = subprocess.run(["ndisasm", "-b16", "-"], input=blob,
                       capture_output=True, timeout=30)
    import re
    marks = []
    for ln in r.stdout.decode('latin1').splitlines():
        m = re.match(r'^([0-9A-F]{8})  (\S+)\s+(.*)$', ln)
        if m: marks.append((int(m.group(1), 16), m.group(3)))
    # ndisasm renders wait, lock and the segment overrides joined to the
    # instruction after them; this decoder treats them separately. The stream
    # of boundaries is the same either way, so those lines are skipped rather
    # than counted as disagreements about length.
    JOINED = ("wait", "lock", "rep", "repe", "repne", "repz", "repnz",
              "cs", "ds", "es", "ss", "fs", "gs", "a16", "a32", "o16", "o32")
    agree = disagree = bail = skip = 0
    for k in range(len(marks) - 1):
        off, txt = marks[k]
        if txt.startswith("db 0x"): continue
        # bare too: where ndisasm cannot decode what follows a prefix it
        # reports the prefix alone, which is not a claim about length
        if txt.split()[0] in JOINED:
            skip += 1; continue
        want = marks[k+1][0] - off
        got = length(blob, off)
        if got is None: bail += 1
        elif got == want: agree += 1
        else: disagree += 1
    ok(disagree == 0, f"{agree} random instructions agree, {disagree} do not, "
                      f"{bail} unknown, {skip} prefix-joined")
    ok(agree > 1000, f"the fuzz corpus produced {agree} comparable instructions")
except (FileNotFoundError, subprocess.TimeoutExpired):
    if VERBOSE: print("   skipped -- no ndisasm")

# ==========================================================================
print()
if fails:
    print(f"{len(fails)} of {checks} checks FAILED")
    for f in fails: print("   ", f)
    sys.exit(1)
print(f"all {checks} checks pass")
