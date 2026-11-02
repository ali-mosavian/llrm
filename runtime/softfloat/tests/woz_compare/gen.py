#!/usr/bin/env python3
"""Operand tables for the Woz-vs-ours comparison. Both sides get the same numeric values (23-bit mantissas are exact in the
Woz 24-bit format and in binary32). Writes, under OUT: woz_<op>.inc (ca65 .byte tables, X2 M2 X1 M1 per record),
ours_<op>.h (C tables of binary32 bits), expect_<op>.txt (exact result as a Python float, for the Woz decode check).
Usage: gen.py OUT [N]"""
import os, random, struct, sys

OUT = sys.argv[1]; N = int(sys.argv[2]) if len(sys.argv) > 2 else 64
random.seed(20261102)

def f32bits(v): return struct.unpack('<I', struct.pack('<f', v))[0]

def woz(v):
    """(E, M) with value = M/2^22 * 2^(E-128), M a 24-bit two's complement mantissa."""
    if v == 0: return 0, 0
    a = abs(v); e = 0
    while a >= 2: a /= 2; e += 1
    while a < 1: a *= 2; e -= 1
    m = int(a * (1 << 22))
    if v < 0: m = (-m) & 0xFFFFFF
    return e + 128, m

def rec(v1, v2):
    e1, m1 = woz(v1); e2, m2 = woz(v2)
    return [e2, m2 >> 16, (m2 >> 8) & 255, m2 & 255, e1, m1 >> 16, (m1 >> 8) & 255, m1 & 255]   # X2 M2 X1 M1, the zero-page order from $F4

def val(lo, hi):
    k = random.randrange(1 << 22, 1 << 23); e = random.randint(lo, hi)
    return random.choice((-1, 1)) * k * 2.0 ** (e - 22)

ops = {}
ops['add'] = [(val(-6, 6), val(-6, 6)) for _ in range(N)]
ops['sub'] = [(val(-6, 6), val(-6, 6)) for _ in range(N)]
ops['mul'] = [(val(-6, 6), val(-6, 6)) for _ in range(N)]
ops['div'] = [(val(-6, 6), val(-6, 6)) for _ in range(N)]          # (dividend, divisor)
ops['i2f'] = [(random.choice((-1, 1)) * random.randint(1, 1 << random.randint(1, 14)), 0) for _ in range(N)]   # log-uniform: the normalize loop is data dependent
ops['f2i'] = [(val(0, 13), 0) for _ in range(N)]
exp = {'add': lambda a, b: a + b, 'sub': lambda a, b: a - b, 'mul': lambda a, b: a * b, 'div': lambda a, b: a / b,
       'i2f': lambda a, b: float(a), 'f2i': lambda a, b: float(int(a))}
for op, pairs in ops.items():
    with open(f'{OUT}/woz_{op}.inc', 'w') as f:
        for a, b in pairs:
            if op in ('div', 'sub'): r = rec(b, a)   # FDIV: divisor in FP1, dividend in FP2; FSUB computes FP2 - FP1 (the listing, not its prose example)
            elif op == 'i2f': r = [0, 0, 0, 0, 0, (a >> 8) & 255, a & 255, 0]   # int16 in M1:M1+1
            else: r = rec(a, b)
            f.write('.byte ' + ','.join(str(x) for x in r) + '\n')
    with open(f'{OUT}/ours_{op}.h', 'w') as f:
        f.write('static const uint32_t A[%d] = {%s};\n' % (N, ','.join('0x%08Xu' % (f32bits(a) if op != 'i2f' else a & 0xFFFFFFFF) for a, _ in pairs)))
        f.write('static const uint32_t B[%d] = {%s};\n' % (N, ','.join('0x%08Xu' % f32bits(b) for _, b in pairs)))
        f.write('static const uint32_t W[%d] = {%s};\n' % (N, ','.join('0x%08Xu' % (f32bits(float(exp[op](a, b))) if op != 'f2i' else int(a) & 0xFFFFFFFF) for a, b in pairs)))
    with open(f'{OUT}/expect_{op}.txt', 'w') as f:
        for a, b in pairs: f.write(repr(float(exp[op](a, b))) + '\n')
