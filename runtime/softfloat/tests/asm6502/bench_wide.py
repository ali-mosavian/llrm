#!/usr/bin/env python3
"""bench_wide.py [N] [OPS]: mean and worst cycles per call of the 6502 f32 add/sub/mul/div, ours (sim65, one run per vector with the
driver's start index and count patched into the binary) and llvm-mos (compiler-rt softfloat, mos-sim, clock read around each call),
on N random operand pairs from gen.py (its 361 special-value pairs dropped; fixed seed). Cycles are call minus a bare copy/rts."""
import os, re, subprocess, sys, tempfile
here = os.path.dirname(os.path.abspath(__file__)); I = os.path.join(here, '../../src/6502')
N = int(sys.argv[1]) if len(sys.argv) > 1 else 2000
OPS = sys.argv[2].split() if len(sys.argv) > 2 else ['add', 'sub', 'mul', 'div']
MOS = os.environ.get('LLVM_MOS', os.path.expanduser('~/scratch/fetch-llvm-mos-sdk/x/llvm-mos')) + '/bin'
T = tempfile.mkdtemp()
def sh(*a, **k): return subprocess.run(a, cwd=k.get('cwd', here), capture_output=True, text=True)
for f in ('sf32', 'sf32_muldiv'):
    assert sh('ca65', '-t', 'sim6502', '-I', I, '-DSTACKB=1', f'{I}/{f}.s', '-o', f'{T}/{f}.o').returncode == 0
open(f'{T}/stub.s', 'w').write('.export __addsf3,__subsf3,__mulsf3,__divsf3\n__addsf3:\n__subsf3:\n__mulsf3:\n__divsf3:\n rts\n')
sh('ca65', '-t', 'sim6502', f'{T}/stub.s', '-o', f'{T}/stub.o')
def ours(op):
    gen = sh('python3', 'gen.py', op, str(N + 361)).stdout.split('\n')
    recs = [l for l in gen if l.strip().startswith('.dword')][361:]
    open(f'{T}/vec.inc', 'w').write('nrec = %d\nvec:\n%s\n' % (len(recs), '\n'.join(recs)))
    open(f'{T}/op.inc', 'w').write(f'.import __{op}sf3\nOP = __{op}sf3\n')
    assert sh('ca65', '-t', 'sim6502', '-U', '-I', T, f'{here}/drv.s', '-o', f'{T}/drv.o').returncode == 0
    for k, objs in (('s', [f'{T}/stub.o']), ('r', [f'{T}/sf32.o', f'{T}/sf32_muldiv.o'])):
        assert sh('ld65', '-t', 'sim6502', f'{T}/drv.o', *objs, 'sim6502.lib', '-o', f'{T}/{k}.bin').returncode == 0
    pat = bytes([0xA9, 0, 0x85, 0xA0, 0xA9, 0, 0x85, 0xA1, 0xA9, 0, 0x85, 0xA2, 0xA9, 0, 0x85, 0xA3])
    def find(data):
        for i in range(len(data) - 16):
            d = data[i:i + 16]
            if d[0] == 0xA9 and d[2:4] == b'\x85\xa0' and d[4] == 0xA9 and d[6:8] == b'\x85\xa1' and d[8] == 0xA9 and d[10:12] == b'\x85\xa2' and d[12] == 0xA9 and d[14:16] == b'\x85\xa3':
                return i
        raise SystemExit('driver prologue not found')
    base = {}
    for k in 'sr':
        data = bytearray(open(f'{T}/{k}.bin', 'rb').read()); o = find(data); base[k] = (data, o)
    vec0 = base['r'][0][base['r'][1] + 1] | base['r'][0][base['r'][1] + 5] << 8
    out = {}
    for k in 'sr':
        res = []
        data, o = base[k]
        vec0 = data[o + 1] | data[o + 5] << 8
        for i in range(len(recs) if k == 'r' else 1):
            d = bytearray(data); a = vec0 + 12 * i
            d[o + 1], d[o + 5] = a & 255, a >> 8
            d[o + 9], d[o + 13] = 1, 0
            open(f'{T}/p.bin', 'wb').write(d)
            p = sh('sim65', '-c', f'{T}/p.bin')
            if not p.stdout.split(): raise SystemExit('sim65 rc=%d out=%r err=%r' % (p.returncode, p.stdout[:200], p.stderr[:200]))
            res.append((int(p.stdout.split()[0]), p.returncode))
        out[k] = res
    b = out['s'][0][0]
    deltas = [c - b for c, _ in out['r']]; fails = sum(1 for _, f in out['r'] if f)
    return sum(deltas) / len(deltas), max(deltas), fails, recs
def mos(op, recs):
    e = {'add': 'x.f + y.f', 'sub': 'x.f - y.f', 'mul': 'x.f * y.f', 'div': 'x.f / y.f'}[op]
    v = [[int(t.strip().lstrip('$'), 16) for t in r.split('.dword')[1].split(',')] for r in recs]
    c = ['#include <stdint.h>', '#include <stdio.h>', 'typedef union { float f; uint32_t u; } U;']
    for n, k in (('A', 0), ('B', 1), ('W', 2)): c.append('static const uint32_t %s[%d] = {%s};' % (n, len(v), ','.join('0x%xu' % t[k] for t in v)))
    c.append('static volatile uint32_t *const clk = (volatile uint32_t *)0xFFF0;')
    c.append('int main(void) { unsigned long sum = 0, sb = 0; uint32_t mx = 0, mb = 0; unsigned fails = 0;')
    c.append(' for (unsigned i = 0; i < %d; i++) { U x, y, r, s; x.u = A[i]; y.u = B[i];' % len(v))
    c.append('  uint32_t t0 = *clk; s.u = x.u; uint32_t t1 = *clk; r.f = %s; uint32_t t2 = *clk;' % e)
    c.append('  uint32_t db = t1 - t0, d = t2 - t1; sum += d; sb += db; if (d > mx) mx = d; if (db > mb) mb = db;')
    c.append('  if (r.u != W[i] && !(((r.u << 1) > 0xFF000000u) && ((W[i] << 1) > 0xFF000000u))) fails++; (void)s; }')
    c.append(' printf("%lu %lu %lu %lu %u\\n", sum, sb, (unsigned long)mx, (unsigned long)mb, fails); return 0; }')
    open(f'{T}/m.c', 'w').write('\n'.join(c))
    p = sh(f'{MOS}/mos-sim-clang', '-Os', '-o', f'{T}/m', f'{T}/m.c'); assert p.returncode == 0, p.stderr
    p = sh(f'{MOS}/mos-sim', f'{T}/m'); s, sb, mx, mb, f = map(int, p.stdout.split())
    return (s - sb) / len(v), mx - mb, f
for op in OPS:
    m, w, f, recs = ours(op); a, b, g = mos(op, recs)
    print(f'{op}: ours mean {m:.0f} worst {w} fails {f} | llvm-mos mean {a:.0f} worst {b} fails {g}   ({len(recs)} pairs)')
