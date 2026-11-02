#!/usr/bin/env python3
"""gen_f32.py OP -> table of a, b, expected binary32 (add sub mul div) from the host; any NaN result is 0x7fc00000."""
import math, random, struct, sys
op = sys.argv[1]
random.seed(3)
f = lambda b: struct.unpack('<f', struct.pack('<I', b))[0]
def bits(x):
    try: return struct.unpack('<I', struct.pack('<f', x))[0]
    except OverflowError: return 0x7f800000 if x > 0 else 0xff800000
def ref(a, b):
    x, y = f(a), f(b)
    if op == 'add': r = x + y
    elif op == 'sub': r = x - y
    elif op == 'mul': r = x * y
    elif y == 0:
        r = float('nan') if x == 0 or x != x else math.copysign(float('inf'), x) * math.copysign(1, y)
    else: r = x / y
    return 0x7fc00000 if r != r else bits(r)
sp = [0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x7f7fffff, 0xff7fffff,
      0x7f800000, 0xff800000, 0x7fc00000, 0x40000000, 0x3f800001, 0x4b000000, 0x4b800000, 0x33800000, 0x34000000, 0x00800001, 0x3f7fffff]
recs = [(a, b) for a in sp for b in sp]
while len(recs) < 700:
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
print("NV = %d\nvec:" % len(recs))
for a, b in recs:
    w = lambda t: [t & 0xffff, t >> 16]
    print("\t.word\t" + ",".join("0x%04x" % t for t in w(a) + w(b) + w(ref(a, b))))
