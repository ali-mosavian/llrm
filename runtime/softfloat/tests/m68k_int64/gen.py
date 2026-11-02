#!/usr/bin/env python3
"""Writes v.h for main.c: uint64 shl shr add sub mul mulw div mod sticky lt on 3000 fixed-seed pairs (expected values from Python).
Run: python3 gen.py > v.h; then CFLAGS_EXTRA="-I. -I../../include" m68k-run-c.sh 68020 Os main.c ops64.c ../../src/int64.c ../../src/int32.c
(m68k-run-c.sh must export labels starting with sf_ as well as __). Expected output: every count 0."""
import random
random.seed(7); M = (1 << 64) - 1
h = lambda v: "0x%08xu,0x%08xu" % (v >> 32, v & 0xffffffff)
print("#define NV 3000\nstatic const uint32_t V[NV][26]={")
for _ in range(3000):
    x = random.getrandbits(random.choice([16, 32, 48, 64])); y = random.getrandbits(random.choice([16, 32, 48, 64])) or 1
    n = random.randint(1, 63)
    r = [x, y, (x << n) & M, x >> n, (x + y) & M, (x - y) & M, (x * y) & M, ((x & 0xffffffff) * (y & 0xffffffff)) & M, x // y, x % y,
         (x >> n) | (1 if (x << (64 - n)) & M else 0)]
    print("{" + ",".join(h(v) for v in r) + ",%d,%d,0,0}," % (n, int(x < y)))
print("};")
