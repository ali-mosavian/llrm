#!/usr/bin/env python3
"""gen.py OP -> ca65-free sdasz80 table: per vector a, b, expected as little-endian words (al ah bl bh el eh)."""
import random, sys
op = sys.argv[1]
random.seed(11)
M = 0xffffffff
def s(x): return x - (1 << 32) if x >> 31 else x
edge = [0, 1, 2, 3, 0xff, 0x100, 0xffff, 0x10000, 0x7fffffff, 0x80000000, 0x80000001, 0xfffffffe, M, 0x12345678, 0x9abcdef0, 0x0000ffff, 0xffff0000]
v = [(a, b) for a in edge for b in edge]
v += [(random.getrandbits(32), random.getrandbits(32)) for _ in range(150)]
v += [(random.getrandbits(32), random.getrandbits(random.choice((8, 16, 24)))) for _ in range(150)]
out = []
for a, b in v:
    if op == 'mul': e = (a * b) & M
    elif op == 'udiv': e = M if b == 0 else a // b
    elif op == 'umod': e = a if b == 0 else a % b
    else:
        if b == 0 or (op in ('div', 'mod') and False): continue
        sa, sb = s(a), s(b)
        q = abs(sa) // abs(sb)
        if (sa < 0) != (sb < 0): q = -q
        e = (q if op == 'div' else sa - q * sb) & M
    out.append((a, b, e))
print("NV = %d" % len(out))
print("vec:")
for a, b, e in out:
    w = lambda x: [x & 0xffff, x >> 16]
    print("\t.word\t" + ",".join("0x%04x" % t for t in w(a) + w(b) + w(e)))
