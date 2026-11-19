#!/usr/bin/env python3
"""gen_cmp_bool.py OP (slt sgt seq) -> gen_f32.py-format table (a, b, expected 0/1 from the host compare) over gen_cmp.py's operands, for SDCC's __fslt/__fsgt/__fseq."""
import random, struct, sys
op = sys.argv[1]
random.seed(5)
f = lambda b: struct.unpack('<f', struct.pack('<I', b))[0]
edge = [0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x3f800001, 0x40000000, 0x7f7fffff, 0xff7fffff,
        0x7f800000, 0xff800000, 0x7fc00000, 0xffc00000, 0x7f800001, 0xff800001, 0x7fffffff, 0x00000002, 0xc0000000]
v = [(a, b) for a in edge for b in edge]
v += [(random.getrandbits(32), random.getrandbits(32)) for _ in range(150)]
v += [(a, a ^ random.choice((0, 1, 0x80000000, 0x100, 0x10000))) for a in (random.getrandbits(32) for _ in range(100))]
print("NV = %d\nvec:" % len(v))
for a, b in v:
    x, y = f(a), f(b)
    r = int({'slt': x < y, 'sgt': x > y, 'seq': x == y}[op])
    w = lambda t: [t & 0xffff, t >> 16]
    print("\t.word\t" + ",".join("0x%04x" % t for t in w(a) + w(b) + [r, 0]))
