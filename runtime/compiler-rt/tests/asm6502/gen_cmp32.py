# usage: gen_cmp32.py edge|rnd start count -> vec.inc records a (4), b (4), the eq/ne/lt/le/cmp result (1), the ge/gt result (1), the contract of
# config/6502/cmp32.s (compiler-rt: -1, 0, 1; unordered is 1 for the first group and -1 for the second). edge: every pair of special values;
# rnd: random bit patterns, sign and exponent biased.
import math, random, struct, sys
kind, start, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
def f(v): return struct.unpack('<f', struct.pack('<I', v))[0]
E = {0, 1 << 31, 1, (1 << 31) | 1, 0x007FFFFF, 0x00800000, 0x3F800000, 0xBF800000, 0x7F7FFFFF, 0xFF7FFFFF, 0x7F800000, 0xFF800000, 0x7F800001,
     0xFF800001, 0x7FC00000, 0xFFC00000, 0x7FFFFFFF, 0xFFFFFFFF, 0x7F80000F, 0x7F800100, 0x3F800100}
for byte in range(4):
    E.add(0x3F812345 ^ (1 << (8 * byte))); E.add(0xBF812345 ^ (1 << (8 * byte)))
E.add(0x3F812345); E.add(0xBF812345)
E = sorted(E)
def pair(i):
    if kind == 'edge': return E[i // len(E)], E[i % len(E)]
    r = random.Random(i)
    def one():
        v = r.getrandbits(32)
        k = r.randrange(6)
        if k == 0: v &= ~(0xFF << 23); v |= r.choice([0, 0xFF, 0x7F, 0x80]) << 23
        if k == 1: v = (v & (0xFFFF << 16)) | (r.getrandbits(8) if r.random() < .5 else 0)
        return v
    a = one()
    return a, (a ^ (1 << r.randrange(32))) if r.random() < .3 else one()
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
    print("  .byte " + ",".join(str(x) for x in b''.join(v.to_bytes(4, 'little') for v in (a, b)) + bytes([p & 255, q & 255])))
