# usage: gen_cmp64.py edge|rnd start count -> vec.inc records a (8), b (8), the eq/ne/lt/le/cmp result (1), the ge/gt result (1), the contract of
# config/6502/cmp64.s (compiler-rt: -1, 0, 1; unordered is 1 for the first group and -1 for the second). edge: every pair of special values
# (zeros, infinities, NaNs, denormals, the extremes, one-byte differences at every byte); rnd: random bit patterns, sign and exponent biased.
import math, random, struct, sys
kind, start, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
def f(v): return struct.unpack('<d', struct.pack('<Q', v))[0]
E = {0, 1 << 63, 1, (1 << 63) | 1, 0x000FFFFFFFFFFFFF, 0x0010000000000000, 0x3FF0000000000000, 0xBFF0000000000000, 0x7FEFFFFFFFFFFFFF,
     0xFFEFFFFFFFFFFFFF, 0x7FF0000000000000, 0xFFF0000000000000, 0x7FF0000000000001, 0xFFF0000000000001, 0x7FF8000000000000, 0xFFF8000000000000,
     0x7FFFFFFFFFFFFFFF, 0xFFFFFFFFFFFFFFFF, 0x7FF000000000000F, 0x7FF0000000000100, 0x3FF0000000000100}
for byte in range(8):
    E.add(0x3FF0123456789ABC ^ (1 << (8 * byte))); E.add(0xBFF0123456789ABC ^ (1 << (8 * byte)))
E.add(0x3FF0123456789ABC); E.add(0xBFF0123456789ABC)
E = sorted(E)
def pair(i):
    if kind == 'edge': return E[i // len(E)], E[i % len(E)]
    r = random.Random(i)
    def one():
        v = r.getrandbits(64)
        k = r.randrange(6)
        if k == 0: v &= ~(0x7FF << 52); v |= r.choice([0, 0x7FF, 0x3FF, 0x400]) << 52
        if k == 1: v = (v & (0xFFFF << 48)) | (r.getrandbits(8) if r.random() < .5 else 0)
        return v
    a = one()
    return a, (a ^ (1 << r.randrange(64))) if r.random() < .3 else one()
total = len(E) ** 2 if kind == 'edge' else 50000
def expect(a, b):
    x, y = f(a), f(b)
    if math.isnan(x) or math.isnan(y): return 1, -1
    r = -1 if x < y else (0 if x == y else 1)
    return r, r
recs = [pair(i) for i in range(start, min(start + count, total))]
print("nrec = %d\nvec:" % len(recs))
for a, b in recs:
    p, q = expect(a, b)
    print("  .byte " + ",".join(str(x) for x in b''.join(v.to_bytes(8, 'little') for v in (a, b)) + bytes([p & 255, q & 255])))
