#!/usr/bin/env python3
"""check_sqtab.py: the SF_MUL_LUT/SF_DIV_LUT tables as linked: quarter squares give x*y for all byte pairs; the reciprocals
never overestimate a quotient digit, by at most 3 (every top byte, 2^16 remainder tops); SF_MUL_NIB nibt[x << 4 | y] = x*y."""
import os, subprocess, tempfile
here = os.path.dirname(os.path.abspath(__file__)); I = os.path.join(here, '../../config/6502'); T = tempfile.mkdtemp()
def sh(*a): subprocess.run(a, cwd=here, check=True, capture_output=True)
sh('ca65', '-g', '-t', 'sim6502', '-I', I, '-DSTACKB=1', '-DSF_MUL_LUT=1', '-DSF_DIV_LUT=1', f'{I}/sf32_muldiv.s', '-o', f'{T}/md.o')
for f in ('sf32', 'sf32_conv'): sh('ca65', '-g', '-t', 'sim6502', '-I', I, '-DSTACKB=1', f'{I}/{f}.s', '-o', f'{T}/{f}.o')
open(f'{T}/m.s', 'w').write('.export _main\n_main: rts\n'); sh('ca65', '-t', 'sim6502', f'{T}/m.s', '-o', f'{T}/m.o')
sh('ld65', '-C', 'sim6502_lut.cfg', '-Ln', f'{T}/l.txt', f'{T}/m.o', f'{T}/md.o', f'{T}/sf32.o', f'{T}/sf32_conv.o', 'sim6502.lib', '-o', f'{T}/t.bin')
b = open(f'{T}/t.bin', 'rb').read(); load = b[8] | b[9] << 8
lab = {l.split()[2].lstrip('.'): int(l.split()[1], 16) for l in open(f'{T}/l.txt')}
tab = {k: [b[12 + lab[k] - load + i] for i in range(512)] for k in ('q1lo', 'q1hi', 'q2lo', 'q2hi')}
assert all(lab[k] & 255 == 0 for k in tab), 'tables must be page aligned'
q = lambda k, n: tab[k + 'lo'][n] | tab[k + 'hi'][n] << 8
bad = sum(1 for x in range(256) for y in range(256) if (q('q1', x + y) - q('q2', 255 - x + y)) & 0xffff != x * y)
rt = [b[12 + lab['rtab'] - load + i] for i in range(128)]
for d in range(128, 256):
    for x in range(0, 1 << 16, 7):
        if x >= d << 8: break
        lo = x * rt[d - 128] >> 15
        q_min, q_max = x * 256 // ((d + 1) << 8), (x + 1) * 256 // (d << 8)
        if not (q_max - 3 <= lo <= q_min): bad += 1
sh('ca65', '-g', '-t', 'sim6502', '-I', I, '-DSTACKB=1', '-DSF_MUL_NIB=1', f'{I}/sf32_muldiv.s', '-o', f'{T}/md.o')
sh('ld65', '-C', 'sim6502_lut.cfg', '-Ln', f'{T}/l.txt', f'{T}/m.o', f'{T}/md.o', f'{T}/sf32.o', f'{T}/sf32_conv.o', 'sim6502.lib', '-o', f'{T}/t.bin')
b = open(f'{T}/t.bin', 'rb').read(); load = b[8] | b[9] << 8
lab = {l.split()[2].lstrip('.'): int(l.split()[1], 16) for l in open(f'{T}/l.txt')}
assert lab['nibt'] & 255 == 0, 'nibt must be page aligned'
bad += sum(1 for x in range(16) for y in range(16) if b[12 + lab['nibt'] - load + (x << 4 | y)] != x * y)
print('sqtab fails=%d' % bad)
