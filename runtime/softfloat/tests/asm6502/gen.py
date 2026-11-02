# usage: gen.py add|sub|mul|div n  -> ca65 records a,b,expected (3 x .dword); a NaN expectation is $7fc00000, any NaN passes
import struct, random, sys
op, n = sys.argv[1], int(sys.argv[2])
f = lambda b: struct.unpack('<f', struct.pack('<I', b))[0]
def bits(x):
    try: return struct.unpack('<I', struct.pack('<f', x))[0]
    except OverflowError: return 0x7f800000 if x > 0 else 0xff800000
import math
def ref(a, b):
    x, y = f(a), f(b)
    if op == 'add': r = x + y
    elif op == 'sub': r = x - y
    elif op == 'mul': r = x * y
    elif y == 0:
        r = float('nan') if x == 0 or x != x else math.copysign(float('inf'), x) * math.copysign(1, y)
    else: r = x / y
    return 0x7fc00000 if r != r else bits(r)
random.seed(1)
sp = [0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x7f7fffff, 0xff7fffff,
      0x7f800000, 0xff800000, 0x7fc00000, 0x40000000, 0x3f800001, 0x4b000000, 0x4b800000, 0x33800000, 0x34000000]
recs = [(a, b) for a in sp for b in sp]
while len(recs) < n:
    a = random.getrandbits(32)
    if random.random() < .12: a &= 0x807fffff                      # denormal or zero
    ea = (a >> 23) & 255
    if op in ('mul', 'div'):
        c = random.choice([3, 30, 130]); d = random.randint(-c, c)
        eb = (127 - ea + d) & 255 if op == 'mul' else (ea + d) & 255
        b = (random.getrandbits(32) & 0x807fffff) | eb << 23
        if random.random() < .12: b &= 0x807fffff
        if random.random() < .15: a = (a & 0xff800000) | random.choice([0, 0x7fffff, 0x400000, 0x7ffffe, 1])
        if random.random() < .15: b = (b & 0xff800000) | random.choice([0, 0x7fffff, 0x400000, 0x7ffffe, 1])
    else:
        d = random.choice([40, 30, 3, 2])
        b = random.getrandbits(32) if random.random() < .3 else (random.getrandbits(32) & 0x807fffff) | ((((a >> 23) & 255) + random.randint(-d, d)) & 255) << 23
    recs.append((a, b))
print("nrec = %d\nvec:" % len(recs))
for a, b in recs:
    print("  .dword $%08x, $%08x, $%08x" % (a, b, ref(a, b)))
