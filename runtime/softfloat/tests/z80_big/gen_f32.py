#!/usr/bin/env python3
"""gen_f32.py OP [edge] -> table of a, b, expected binary32 (add sub mul div) from the host; any NaN result is 0x7fc00000.
edge: 1800 records near the edges instead: add/sub exponent gaps 20..34, mul/div results near the denormal range, ftoi/ftou exponents 126..160."""
import math, random, struct, sys
op = sys.argv[1]
random.seed(99)
f = lambda b: struct.unpack('<f', struct.pack('<I', b))[0]
def bits(x):
    try: return struct.unpack('<I', struct.pack('<f', x))[0]
    except OverflowError: return 0x7f800000 if x > 0 else 0xff800000
def ref(a, b):
    x, y = f(a), f(b)
    if op == 'itof': return bits(float(a - (1 << 32) if a >> 31 else a))
    if op == 'utof': return bits(float(a))
    if op in ('ftoi', 'ftou'):
        if x != x: return 0
        lo, hi = (-(1 << 31), (1 << 31) - 1) if op == 'ftoi' else (0, (1 << 32) - 1)
        t = max(lo, min(hi, int(x))) if abs(x) != float('inf') else (hi if x > 0 else lo)
        return t & 0xffffffff
    if op == 'add': r = x + y
    elif op == 'sub': r = x - y
    elif op == 'mul': r = x * y
    elif y == 0:
        r = float('nan') if x == 0 or x != x else math.copysign(float('inf'), x) * math.copysign(1, y)
    else: r = x / y
    return 0x7fc00000 if r != r else bits(r)
sp = [0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x7f7fffff, 0xff7fffff,
      0x7f800000, 0xff800000, 0x7fc00000, 0x40000000, 0x3f800001, 0x4b000000, 0x4b800000, 0x33800000, 0x34000000, 0x00800001, 0x3f7fffff]
conv = op in ('itof', 'utof', 'ftoi', 'ftou')
recs = [(a, 0) for a in sp + [0x4f000000, 0xcf000000, 0x4effffff, 0xcf000001, 0x4f800000, 0x4f7fffff, 0x3f7fffff, 0xbf7fffff, 0x3fc00000, 0xbfc00000, 0x4b7fffff, 0x7fffffff, 0xffffffff, 0x7fa00000, 0x01000001, 0x7fffff80, 0x80000000]] if conv else [(a, b) for a in sp for b in sp]
while conv and len(recs) < 9000:
    if op in ('itof', 'utof'):
        a = random.getrandbits(random.choice([3, 8, 16, 24, 25, 26, 31, 32])) if random.random() < .8 else random.choice([0x80000000, 0x7fffffff, 0xffffffff, 0x01000001, 0x01000003, 0x7fffff80, 0x7fffffc0])
        if random.random() < .3: a = (-a) & 0xffffffff
    else:
        a = (random.getrandbits(32) & 0x807fffff) | random.randint(110, 165) << 23
        if random.random() < .1: a = random.getrandbits(32)
    recs.append((a, 0))
while not conv and len(recs) < 9000:
    a = random.getrandbits(32)
    if random.random() < .12: a &= 0x807fffff
    ea = (a >> 23) & 255
    if op in ('mul', 'div'):
        c = random.choice([3, 30, 130]); d = random.randint(-c, c)
        eb = (127 - ea + d) & 255 if op == 'mul' else (ea + d) & 255
        b = (random.getrandbits(32) & 0x807fffff) | eb << 23
        if random.random() < .12: b &= 0x807fffff
        if random.random() < .15: a = (a & 0xff800000) | random.choice([0, 0x7fffff, 0x400000, 0x7ffffe, 1])
        if random.random() < .15: b = (b & 0xff800000) | random.choice([0, 0x7fffff, 0x400000, 0x7ffffe, 1])
    else:
        d = random.choice([40, 30, 3, 2, 1])
        b = random.getrandbits(32) if random.random() < .3 else (random.getrandbits(32) & 0x807fffff) | ((((a >> 23) & 255) + random.randint(-d, d)) & 255) << 23
    recs.append((a, b))
if sys.argv[2:] == ['edge']:
    m = lambda: random.choice([0, 1, 0x7fffff, 0x400000, 0x7ffffe, random.getrandbits(23)])
    sg = lambda: random.choice([0, 1 << 31])
    recs = []
    while len(recs) < 1800:
        if op in ('add', 'sub'):
            ea = random.randint(36, 254); d = random.randint(20, 34)
            a, b = sg() | ea << 23 | m(), sg() | (ea - d) << 23 | m()
            if random.random() < .5: a, b = b, a
        elif op in ('mul', 'div'):
            ea = random.randint(0, 160); r = random.randint(-33, 3)
            eb = r + 127 - ea if op == 'mul' else ea + 127 - r
            if not 0 <= eb <= 254: continue
            a, b = sg() | ea << 23 | m(), sg() | eb << 23 | m()
        elif op in ('ftoi', 'ftou'):
            a = sg() | random.randint(126, 160) << 23 | m(); b = 0
        else:
            a = random.getrandbits(random.randint(1, 32)); b = 0
        recs.append((a, b))
print("NV = %d\nvec:" % len(recs))
for a, b in recs:
    w = lambda t: [t & 0xffff, t >> 16]
    print("\t.word\t" + ",".join("0x%04x" % t for t in w(a) + w(b) + w(ref(a, b))))
