# usage: gen.py add|sub|mul|div|itof|utof|ftoi|ftou n [edge]  -> (edge: n records with add/sub exponent gaps 20..34,
# mul/div results near the denormal range, ftoi/ftou exponents 126..160) ca65 records a,b,expected (3 x .dword); a NaN expectation is $7fc00000, any NaN passes
import struct, random, sys
op, n = sys.argv[1], int(sys.argv[2])
f = lambda b: struct.unpack('<f', struct.pack('<I', b))[0]
def bits(x):
    try: return struct.unpack('<I', struct.pack('<f', x))[0]
    except OverflowError: return 0x7f800000 if x > 0 else 0xff800000
import math
def ref(a, b):
    x, y = f(a), f(b)
    if op == 'itof': return bits(float(a - (1 << 32) if a >> 31 else a))
    if op == 'utof': return bits(float(a))
    if op in ('ftoi', 'ftou'):
        if x != x: return 0
        lo, hi = (-(1 << 31), (1 << 31) - 1) if op == 'ftoi' else (0, (1 << 32) - 1)
        t = max(lo, min(hi, int(x))) if abs(x) != float('inf') else (hi if x > 0 else lo)
        return t & 0xffffffff
    if DIRECTED and op in ('add', 'sub', 'mul', 'div'):
        # Rule asserted bit for bit (compiler-rt's, which x86 SSE also follows for operands): a NaN operand is returned with its sign and payload and the
        # quiet bit set, the first NaN operand (a, then b) winning; sub does not flip b's sign. An invalid operation (inf-inf, 0*inf, 0/0, inf/inf)
        # returns the default NaN 0x7fc00000 (x86 hardware returns 0xffc00000 there, so the host is not the reference for that class).
        for v in (a, b):
            if (v >> 23) & 255 == 255 and v & 0x7fffff: return v | 0x400000
        if (x == 0 and math.isinf(y)) or (math.isinf(x) and y == 0):
            if op == 'mul': return 0x7fc00000
    if op == 'add': r = x + y
    elif op == 'sub': r = x - y
    elif op == 'mul': r = x * y
    elif y == 0:
        r = float('nan') if x == 0 or x != x else math.copysign(float('inf'), x) * math.copysign(1, y)
    else: r = x / y
    return 0x7fc00000 if r != r else bits(r)
random.seed(1)
DIRECTED = sys.argv[3:] == ['directed']
sp = [0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x7f7fffff, 0xff7fffff,
      0x7f800000, 0xff800000, 0x7fc00000, 0x40000000, 0x3f800001, 0x4b000000, 0x4b800000, 0x33800000, 0x34000000]
recs = [(a, b) for a in sp for b in sp] if op in ('add', 'sub', 'mul', 'div') else [(a, 0) for a in sp + [0x4f000000, 0xcf000000, 0x4eff_ffff, 0xcf000001, 0x4f800000, 0x4f7fffff, 0x3f7fffff, 0xbf7fffff, 0x3fc00000, 0xbfc00000, 0x4b7fffff, 0x7fffffff, 0xffffffff, 0x7fa00000]]
while len(recs) < n and op in ('itof', 'utof', 'ftoi', 'ftou'):
    if op in ('itof', 'utof'):
        a = random.getrandbits(random.choice([3, 8, 16, 24, 25, 26, 31, 32])) if random.random() < .8 else random.choice([0x80000000, 0x7fffffff, 0xffffffff, 0x01000001, 0x01000003, 0x7fffff80, 0x7fffffc0])
        if random.random() < .3: a = (-a) & 0xffffffff
    else:
        a = (random.getrandbits(32) & 0x807fffff) | random.randint(110, 165) << 23
        if random.random() < .1: a = random.getrandbits(32)
    recs.append((a, 0))
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
if sys.argv[3:] == ['edge']:
    m = lambda: random.choice([0, 1, 0x7fffff, 0x400000, 0x7ffffe, random.getrandbits(23)])
    sg = lambda: random.choice([0, 1 << 31])
    recs = []
    while len(recs) < n:
        if op in ('add', 'sub'):
            ea = random.randint(36, 254); d = random.randint(20, 34)
            a, b = sg() | ea << 23 | m(), sg() | (ea - d) << 23 | m()
            if random.random() < .5: a, b = b, a
        elif op in ('mul', 'div'):
            ea = random.randint(0, 160); r = random.randint(-33, 3)
            eb = r + 127 - ea if op == 'mul' else ea + 127 - r
            if not 0 <= eb <= 254: continue
            a, b = sg() | ea << 23 | m(), sg() | eb << 23 | m()
        elif op in ('ftoi', 'ftou'):
            a = sg() | random.randint(126, 160) << 23 | m(); b = 0
        else:
            a = random.getrandbits(random.randint(1, 32)); b = 0
        recs.append((a, b))
if sys.argv[3:] == ['directed']:
    # NaN classes: quiet/signalling, either sign, payloads, in either operand, and the invalid operations (inf-inf, 0*inf, 0/0, inf/inf).
    nans = [0x7fc00000, 0xffc00000, 0x7fa00000, 0xffa00000, 0x7f800001, 0xff800001, 0x7fffffff, 0xffffffff, 0x7fc12345, 0xffdeadbe]
    fin = [0, 0x80000000, 0x3f800000, 0xbf800000, 0x7f800000, 0xff800000, 0x00000001, 0x7f7fffff]
    recs = [(a, b) for a in nans for b in nans + fin] + [(b, a) for a in nans for b in fin]
    recs += [(0x7f800000, 0xff800000), (0xff800000, 0x7f800000), (0x7f800000, 0x7f800000), (0, 0x7f800000), (0x80000000, 0xff800000), (0, 0), (0x80000000, 0), (0x7f800000, 0xff800000)]
    # round-to-even ties: mul products whose dropped bits are exactly half an ulp (both normalisation shifts), and denormal-result ties by 2.0 / 0.5.
    while len(recs) < n and op in ('mul', 'div'):
        if op == 'mul':
            # v2(ma*mb) = s-1 makes the dropped bits exactly half an ulp; build the factors from their 2-adic parts (rejection sampling needs ~2^23 tries)
            s = random.choice([23, 24]); i = random.randint(0, min(23, s - 1)); j = s - 1 - i
            if j > 23: continue
            def fac(v):
                o = (random.getrandbits(24 - v) | 1 << (23 - v)) | 1
                return o << v
            ma, mb = fac(i), fac(j)
            p = ma * mb
            if p & ((1 << (24 if p >> 47 else 23)) - 1) != 1 << ((24 if p >> 47 else 23) - 1): continue
            ea, eb = random.randint(100, 150), random.randint(100, 150)
            recs.append((sg_ := random.choice([0, 1 << 31]) | ea << 23 | ma & 0x7fffff, random.choice([0, 1 << 31]) | eb << 23 | mb & 0x7fffff))
        else:
            m = random.getrandbits(23) | 1
            recs.append((random.choice([0, 1 << 31]) | random.choice([0, 1, 2, 5]) << 23 | m, 0x40000000))
    while len(recs) < n and op in ('add', 'sub'):
        ma = random.getrandbits(23) | 1; ea = random.randint(30, 200)       # a + b where b is exactly half an ulp of a: tie to even
        recs.append((ea << 23 | ma, (ea - 24) << 23))
    recs = recs[:n] if op in ('mul', 'div', 'add', 'sub') else recs
    # conversions: NaN/inf/zero/denormal classes, range edges, and int->float exact half-ulp ties (24-bit significand, dropped bits = 1 << (L-25))
    if op in ('ftoi', 'ftou'):
        recs = [(v, 0) for v in nans + [0, 0x80000000, 0x7f800000, 0xff800000, 1, 0x80000001, 0x007fffff, 0x807fffff, 0x4f000000, 0xcf000000, 0x4f800000, 0xcf800000, 0x4effffff, 0xceffffff, 0x4f7fffff, 0x3f7fffff, 0xbf7fffff, 0xbf800000]]
        while len(recs) < n: recs.append(((random.getrandbits(32) & 0x807fffff) | random.randint(110, 165) << 23, 0))
    if op in ('itof', 'utof'):
        recs = [(v, 0) for v in (0, 1, 0xffffffff, 0x80000000, 0x7fffffff, 0x80000001)]
        while len(recs) < n:
            L = random.randint(25, 32); m = random.getrandbits(23) | 1 << 23
            a = (m << (L - 24)) | 1 << (L - 25)
            if random.random() < .5: a = (-a) & 0xffffffff if op == 'itof' else a
            recs.append((a & 0xffffffff, 0))
print("nrec = %d\nvec:" % len(recs))
for a, b in recs:
    print("  .dword $%08x, $%08x, $%08x" % (a, b, ref(a, b)))
