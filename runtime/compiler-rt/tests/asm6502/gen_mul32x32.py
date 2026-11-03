# usage: gen_mul32x32.py pat|rnd start count -> vec.inc records a (4), b (4), a*b (8), little endian
# pat: every pair of operands whose bytes are each 00, 01, 7F, 80 or FF (625 x 625); rnd: 100000 random pairs, bit widths mixed.
import random, sys
kind, start, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
B = (0x00, 0x01, 0x7F, 0x80, 0xFF)
P = [B[i % 5] | B[i // 5 % 5] << 8 | B[i // 25 % 5] << 16 | B[i // 125] << 24 for i in range(625)]
def pair(i):
    if kind == 'pat': return P[i // 625], P[i % 625]
    r = random.Random(i)
    return tuple(r.getrandbits(r.choice((8, 16, 24, 32, 32, 32))) for _ in range(2))
total = 625 * 625 if kind == 'pat' else 100000
recs = [pair(i) for i in range(start, min(start + count, total))]
print("nrec = %d\nvec:" % len(recs))
for a, b in recs:
    print("  .byte " + ",".join(str(x) for x in a.to_bytes(4, 'little') + b.to_bytes(4, 'little') + (a * b).to_bytes(8, 'little')))
