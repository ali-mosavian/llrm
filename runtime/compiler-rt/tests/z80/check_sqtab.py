#!/usr/bin/env python3
"""check_sqtab.py: the SF_MUL_LUT quarter-square tables of config/z80/sf32_arith.s as linked give x*y for all byte pairs; SF_MUL_NIB's nibt[x << 4 | y] = x*y."""
import os, subprocess, tempfile
here = os.path.dirname(os.path.abspath(__file__)); S = os.path.join(here, '../../config/z80/sf32_arith.s'); T = tempfile.mkdtemp()
def sh(*a): subprocess.run(a, cwd=T, check=True, capture_output=True)
open(f'{T}/defs.s', 'w').write('SF_MUL_LUT = 1\n'); open(f'{T}/a.s', 'w').write(open(S).read())
sh('sdasz80', '-o', 'a.rel', 'defs.s', 'a.s')
sh('sdldz80', '-i', '-m', '-b', '_CODE=0x100', '-b', '_SFLUT=0x5000', 't.ihx', 'a.rel')
mem = bytearray(65536)
for l in open(f'{T}/t.ihx'):
    n, a, t = int(l[1:3], 16), int(l[3:7], 16), int(l[7:9], 16)
    if t == 0: mem[a:a + n] = bytes.fromhex(l[9:9 + 2 * n])
q = lambda base, n: mem[base + n] | mem[base + 512 + n] << 8
bad = sum(1 for x in range(256) for y in range(256) if (q(0x5000, x + y) - q(0x5400, 255 - x + y)) & 0xffff != x * y)
open(f'{T}/defs.s', 'w').write('SF_MUL_NIB = 1\n'); sh('sdasz80', '-o', 'a.rel', 'defs.s', 'a.s')
sh('sdldz80', '-i', '-m', '-b', '_CODE=0x100', '-b', '_SFLUT=0x7000', 't.ihx', 'a.rel')
mem = bytearray(65536)
for l in open(f'{T}/t.ihx'):
    n, a, t = int(l[1:3], 16), int(l[3:7], 16), int(l[7:9], 16)
    if t == 0: mem[a:a + n] = bytes.fromhex(l[9:9 + 2 * n])
bad += sum(1 for x in range(16) for y in range(16) if mem[0x7000 + (x << 4 | y)] != x * y)
print('sqtab fails=%d' % bad)
