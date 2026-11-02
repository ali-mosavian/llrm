# usage: gen8.py op chunk -> ca65 table for a in [16*chunk, 16*chunk+16), b in 0..255 (OCP FP8 E5M2, RNE; any NaN passes as $7e)
import sys
from fractions import Fraction as F
op, chunk = sys.argv[1], int(sys.argv[2])
def val(c):
    s, e, m = c >> 7, (c >> 2) & 31, c & 3
    if e == 31: return float('nan') if m else (float('-inf') if s else float('inf'))
    v = F(m, 4) * F(2) ** -14 if e == 0 else (1 + F(m, 4)) * F(2) ** (e - 15)
    return -v if s else v
fin = sorted([(val(c), c) for c in range(0x7c)] + [(F(65536), 0x7c)])
def rnd(v, neg):
    if v == 0: return 0x80 if neg else 0
    a = abs(v)
    if a >= F(61440): return 0xfc if v < 0 else 0x7c  # the tie goes to the even code, infinity
    lo = max((x for x in fin if x[0] <= a), key=lambda x: x[0])
    hi = min((x for x in fin if x[0] >= a), key=lambda x: x[0])
    if lo[0] == hi[0]: c = lo[1]
    else:
        dl, dh = a - lo[0], hi[0] - a
        c = lo[1] if dl < dh else hi[1] if dh < dl else (lo[1] if lo[1] % 2 == 0 else hi[1])
    return c | (0x80 if v < 0 else 0)
def ref(a, b):
    x, y = val(a), val(b)
    nan = (x != x) or (y != y)
    inf = lambda v: v in (float('inf'), float('-inf'))
    if nan: return 0x7e
    if op in ('add', 'sub'):
        if op == 'sub': y = -y
        if inf(x) or inf(y):
            if inf(x) and inf(y) and x != y: return 0x7e
            return 0xfc if (x if inf(x) else y) < 0 else 0x7c
        s = F(x) + F(y)
        if s == 0:
            nx = a & 0x80 != 0; ny = (b & 0x80 != 0) ^ (op == 'sub')
            return 0x80 if (nx and ny and F(x) == 0 and F(y) == 0) else 0
        return rnd(s, s < 0)
    neg = ((a ^ b) & 0x80) != 0
    if op == 'mul':
        if (inf(x) and y == 0) or (inf(y) and x == 0): return 0x7e
        if inf(x) or inf(y): return 0xfc if neg else 0x7c
        return rnd(F(x) * F(y), neg)
    if inf(x) and inf(y): return 0x7e
    if inf(x): return 0xfc if neg else 0x7c
    if inf(y): return 0x80 if neg else 0
    if y == 0: return 0x7e if x == 0 else (0xfc if neg else 0x7c)
    return rnd(F(x) / F(y), neg)
print("A0 = %d" % (16 * chunk))
print("exp8:")
for a in range(16 * chunk, 16 * chunk + 16):
    print("  .byte " + ",".join(str(ref(a, b)) for b in range(256)))
