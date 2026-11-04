# usage: [FP8_ROWS=16] gen8.py op chunk -> ca65 table for a in [ROWS*chunk, ROWS*chunk+ROWS), b in 0..255; ROWS*256 records run (OCP FP8 E5M2, RNE; any NaN passes as $7e)
import sys
from fractions import Fraction as F
op, chunk = (sys.argv[1], int(sys.argv[2])) if len(sys.argv) > 2 else ('mul', 0)
import os
M = int(os.environ.get('FP8_M', 2)); E = 7 - M
SPECIAL = int(os.environ.get('FP8_SPECIAL', 1)); DENORM = int(os.environ.get('FP8_DENORM', 1))
BIAS = (1 << (E - 1)) - 1; EMAX = (1 << E) - 1; INF = EMAX << M; QNAN = INF | (1 << (M - 1))
def val(c):
    s, e, m = c >> 7, (c >> M) & EMAX, c & ((1 << M) - 1)
    if e == EMAX: return float('nan') if m else (float('-inf') if s else float('inf'))
    if e == 0 and not DENORM: v = F(0)
    else: v = F(m, 1 << M) * F(2) ** (1 - BIAS) if e == 0 else (1 + F(m, 1 << M)) * F(2) ** (e - BIAS)
    return -v if s else v
def real(c):
    e, m = c >> M, c & ((1 << M) - 1)
    return F(m, 1 << M) * F(2) ** (1 - BIAS) if e == 0 else (1 + F(m, 1 << M)) * F(2) ** (e - BIAS)
fin = sorted([(real(c), c) for c in range(INF)] + [(F(2) ** (EMAX - BIAS), INF)])
MINN = F(2) ** (1 - BIAS)
def rnd(v, neg):
    if v == 0: return 0x80 if neg else 0
    a = abs(v)
    if not DENORM and a < MINN: return 0x80 if neg else 0
    if a >= fin[-1][0]: return INF | (0x80 if neg else 0)
    lo = max((x for x in fin if x[0] <= a), key=lambda x: x[0])
    hi = min((x for x in fin if x[0] >= a), key=lambda x: x[0])
    if lo[0] == hi[0]: c = lo[1]
    else:
        dl, dh = a - lo[0], hi[0] - a
        c = lo[1] if dl < dh else hi[1] if dh < dl else (lo[1] if lo[1] % 2 == 0 else hi[1])
    return c | (0x80 if neg else 0)
def ref(a, b):
    if not SPECIAL and ((a & 0x7f) >= INF or (b & 0x7f) >= INF): return 0xff
    x, y = val(a), val(b)
    nan = (x != x) or (y != y)
    inf = lambda v: v in (float('inf'), float('-inf'))
    if nan: return QNAN
    if op in ('add', 'sub'):
        if op == 'sub': y = -y
        if inf(x) or inf(y):
            if inf(x) and inf(y) and x != y: return QNAN
            return (INF | 0x80) if (x if inf(x) else y) < 0 else INF
        s = F(x) + F(y)
        if s == 0:
            nx = a & 0x80 != 0; ny = (b & 0x80 != 0) ^ (op == 'sub')
            return 0x80 if (nx and ny and F(x) == 0 and F(y) == 0) else 0
        return rnd(s, s < 0)
    neg = ((a ^ b) & 0x80) != 0
    if op == 'mul':
        if (inf(x) and y == 0) or (inf(y) and x == 0): return QNAN
        if inf(x) or inf(y): return (INF | 0x80) if neg else INF
        return rnd(F(x) * F(y), neg)
    if inf(x) and inf(y): return QNAN
    if inf(x): return (INF | 0x80) if neg else INF
    if inf(y): return 0x80 if neg else 0
    if y == 0: return QNAN if (x == 0 and SPECIAL) else ((INF | 0x80) if neg else INF)
    return rnd(F(x) / F(y), neg)
if __name__ == '__main__':
    rows = int(os.environ.get('FP8_ROWS', 16))
    print("ROWS = %d" % rows)
    print("A0 = %d" % (rows * chunk))
    print("exp8:")
    for a in range(rows * chunk, rows * chunk + rows):
        print("  .byte " + ",".join(str(ref(a, b)) for b in range(256)))
