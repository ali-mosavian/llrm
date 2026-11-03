# usage: gen_di64.py mul|udiv|umod|div|mod n -> records: a (8 bytes), b (8), expected (8), little endian
# (a6:a7 and b follow each other, so rc0:rc1 = record + 6 gives the stack layout of the 6502 convention)
import random, sys
op, n = sys.argv[1], int(sys.argv[2])
random.seed(11)
M = (1 << 64) - 1
def sgn(x): return x - (1 << 64) if x >> 63 else x
def ref(a, b):
    if op == 'mul': return (a * b) & M
    if op in ('udiv', 'umod'):
        if b == 0: return M if op == 'udiv' else a
        return a // b if op == 'udiv' else a % b
    x, y = sgn(a), sgn(b)
    if y == 0: return M if op == 'div' else a
    if x == -(1 << 63) and y == -1: return a if op == 'div' else 0
    q = abs(x) // abs(y); r = abs(x) % abs(y)
    if (x < 0) != (y < 0): q = -q
    if x < 0: r = -r
    return (q if op == 'div' else r) & M
edge = [0, 1, 2, 3, 255, 256, 65535, 65536, M, 1 << 63, (1 << 63) - 1, 0x00000000ffffffff, 0x0000000100000000, 0xffffffff00000000, 0x0123456789abcdef, 0xfedcba9876543210]
recs = [(a, b) for a in edge for b in edge]
def rnd():
    return random.getrandbits(random.choice([1, 4, 8, 12, 16, 24, 32, 40, 48, 56, 64])) if random.random() < .85 else random.choice(edge)
while len(recs) < n:
    a, b = rnd(), rnd()
    if random.random() < .25: a = (-a) & M
    if random.random() < .25: b = (-b) & M
    recs.append((a, b))
print("nrec = %d\nvec:" % len(recs))
for a, b in recs:
    print("  .byte " + ",".join(str(x) for x in a.to_bytes(8, 'little') + b.to_bytes(8, 'little') + ref(a, b).to_bytes(8, 'little')))
