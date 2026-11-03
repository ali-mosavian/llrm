# usage: gen_div64.py edge|rnd start count -> vec.inc records ma (8), mb (8), q (8), r (8), little endian, the contract of config/6502/div64.s:
# q = floor(ma * 2^56 / mb), r = 2 * (ma * 2^56 mod mb) (the 57th step shifts the remainder once more). Operands are normalized
# mantissas in 2^52..2^53-1. edge: every pair of {2^52, 2^52+1, 2^53-1, the byte patterns 00/FF/80/01 runs}; rnd: 50000 random pairs.
import random, sys
kind, start, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
H = 1 << 52
E = sorted({H, H + 1, H + 2, 2 * H - 1, 2 * H - 2, H + (H >> 1), 3 * H // 2 + 1, H | 0xFF, H | 0xFF00, H | 0xFF0000, H | 0xFF000000,
            H | (0xFF << 32), H | (0xFF << 40), H | 0x8080808080808, H | 0x7F7F7F7F7F7F7, 0x1AAAAAAAAAAAAA, 0x15555555555555})
def pair(i):
    if kind == 'edge': return E[i // len(E)], E[i % len(E)]
    r = random.Random(i)
    return H | r.getrandbits(52), H | r.getrandbits(52)
total = len(E) ** 2 if kind == 'edge' else 50000
recs = [pair(i) for i in range(start, min(start + count, total))]
print("nrec = %d\nvec:" % len(recs))
for a, b in recs:
    q, rem = divmod(a << 56, b)
    print("  .byte " + ",".join(str(x) for x in b''.join(v.to_bytes(8, 'little') for v in (a, b, q, 2 * rem))))
