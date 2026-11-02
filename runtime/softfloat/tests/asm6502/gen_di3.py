# usage: gen_di3.py shl|shr|sar n -> records: w0 w1 w2 (6 bytes), w3 (2), count (2), expected (8), little endian
import random, sys
op, n = sys.argv[1], int(sys.argv[2])
random.seed(7)
M64 = (1 << 64) - 1
def ref(a, c):
    if c >= 64: return 0 if op != 'sar' or not (a >> 63) else M64
    if op == 'shl': return (a << c) & M64
    if op == 'shr': return a >> c
    s = a - (1 << 64) if a >> 63 else a
    return (s >> c) & M64
edge = [0, 1, M64, 1 << 63, (1 << 63) - 1, 0x0123456789abcdef, 0xfedcba9876543210, 0x00000000ffffffff, 0xffffffff00000000]
recs = [(a, c) for a in edge for c in list(range(0, 66)) + [127, 128, 255, 256, 0x8000, 0xffff]]
while len(recs) < n:
    recs.append((random.getrandbits(random.choice([8, 16, 32, 48, 64])) if random.random() < .8 else random.choice(edge), random.choice([random.randrange(64), random.randrange(64), random.randrange(80), random.getrandbits(16)])))
print("nrec = %d\nvec:" % len(recs))
for a, c in recs:
    b = a.to_bytes(8, 'little'); r = ref(a, c).to_bytes(8, 'little')
    print("  .byte " + ",".join(str(x) for x in list(b) + list(c.to_bytes(2, 'little')) + list(r)))
