#!/usr/bin/env python3
"""The C translation (lib/fast/fast32_woz.c) against the original machine code on sim65, byte for byte, on the same operands.
Usage: xcheck.py [per-op count]  (TMPDIR for the work dir)"""
import os, random, subprocess, sys, tempfile
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import woz_build as wb

HERE = wb.HERE; SF = os.path.abspath(os.path.join(HERE, '..', '..')); N = int(sys.argv[1]) if len(sys.argv) > 1 else 600
T = tempfile.mkdtemp(prefix='xchk_', dir=os.environ.get('TMPDIR'))
random.seed(1976)
wb.prepare(T)
wb.sh('cc', '-O2', f'-I{SF}/include', f'{HERE}/fzw_run.c', f'{SF}/lib/fast/fast32_woz.c', '-o', f'{T}/fzw_run')

def mant():
    r = random.random()
    if r < 0.15: return random.choice((0x400000, 0x7FFFFF, 0x800000, 0xBFFFFF, 0xC00000, 0xC00001, 0x000001, 0xFFFFFF, 0))
    if r < 0.30: return random.randrange(0, 1 << 24)                          # any, normalized or not
    while True:
        m = random.randrange(0, 1 << 24)
        if ((m >> 23) ^ (m >> 22)) & 1: return m                              # normalized
def expo(): return random.choice((random.randint(100, 156), random.randint(1, 255), random.randint(110, 146)))
def vec(op):
    x1, x2, m1, m2 = expo(), expo(), mant(), mant()
    if op in ('add', 'sub') and random.random() < 0.4: x2 = max(1, min(255, x1 + random.randint(-30, 30)))
    if op in ('add', 'sub') and random.random() < 0.1: m2 = (-m1) & 0xFFFFFF              # cancellation
    if random.random() < 0.05: x1, m1 = 0, 0
    if random.random() < 0.05: x2, m2 = 0, 0
    if op == 'div' and m1 == 0: m1 = 0x400000
    if op == 'i2f': x1, m2, x2 = 0, 0, 0; m1 = (random.getrandbits(16) << 8)
    if op == 'f2i': m2, x2 = 0, 0
    return [x2, m2 >> 16, (m2 >> 8) & 255, m2 & 255, x1, m1 >> 16, (m1 >> 8) & 255, m1 & 255]

total_bad = 0
for op in wb.ENTRY:
    recs = [vec(op) for _ in range(N)]
    lines = '\n'.join('%s %x %x %x %x' % (op, r[0], r[1] << 16 | r[2] << 8 | r[3], r[4], r[5] << 16 | r[6] << 8 | r[7]) for r in recs)
    model = subprocess.run([f'{T}/fzw_run'], input=lines, capture_output=True, text=True).stdout.split()
    keep = [(r, m) for r, m in zip(recs, model) if m != 'ovf']
    raw = wb.run(T, op, [r for r, _ in keep])
    bad = 0
    for k, (r, m) in enumerate(keep):
        got = raw[4 * k:4 * k + 4].hex()
        want = m
        if op == 'f2i': got, want = got[2:6], want[2:6]
        if got != want:
            bad += 1
            if bad <= 4: print('MISMATCH', op, 'rec', ' '.join('%02x' % b for b in r), 'asm', got, 'C', want)
    print('%-4s %5d vectors (%d skipped: the original would take its overflow vector)  mismatches %d' % (op, len(keep), len(recs) - len(keep), bad))
    total_bad += bad
print('workdir', T)
sys.exit(1 if total_bad else 0)
