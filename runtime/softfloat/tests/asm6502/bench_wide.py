#!/usr/bin/env python3
"""bench_wide.py [N] [OPS]: mean and worst cycles per call of the 6502 f32 add/sub/mul/div, ours (sim65, one run per vector with the
driver's start index and count patched into the binary) and llvm-mos (compiler-rt softfloat, mos-sim, clock read around each call),
(env SF_DEFS: extra ca65 flags for the library, e.g. -DSF_MUL_LUT=1; OPS also itof utof ftoi ftou: the four conversions, one operand), on N random operand pairs from gen.py (its 361 special-value pairs dropped; fixed seed). Cycles are call minus a bare copy/rts."""
import os, re, subprocess, sys, tempfile
here = os.path.dirname(os.path.abspath(__file__)); I = os.path.join(here, '../../src/6502')
N = int(sys.argv[1]) if len(sys.argv) > 1 else 2000
OPS = sys.argv[2].split() if len(sys.argv) > 2 else ['add', 'sub', 'mul', 'div']
MOS = os.environ.get('LLVM_MOS', os.path.expanduser('~/scratch/fetch-llvm-mos-sdk/x/llvm-mos')) + '/bin'
T = tempfile.mkdtemp()
def sh(*a, **k): return subprocess.run(a, cwd=k.get('cwd', here), capture_output=True, text=True)
SYM = {'add': '__addsf3', 'sub': '__subsf3', 'mul': '__mulsf3', 'div': '__divsf3', 'itof': '__floatsisf', 'utof': '__floatunsisf', 'ftoi': '__fixsfsi', 'ftou': '__fixunssfsi'}
CONV = ('itof', 'utof', 'ftoi', 'ftou')
for f in ('sf32', 'sf32_muldiv', 'sf32_conv'):
    assert sh('ca65', '-t', 'sim6502', '-I', I, '-DSTACKB=1', *os.environ.get('SF_DEFS', '').split(), f'{I}/{f}.s', '-o', f'{T}/{f}.o').returncode == 0
# The stub returns 0 and the base run's one record expects 0, so the driver takes the same success path as in a library run (a failing record costs ~150 cycles in its report loop).
# The base run links the library too (crt0 initializes its DATA and BSS before main) and calls the stub instead.
open(f'{T}/stub.s', 'w').write('.export stub_op\nstub_op:\n lda #0\n ldx #0\n sta $82\n sta $83\n rts\n')
sh('ca65', '-t', 'sim6502', f'{T}/stub.s', '-o', f'{T}/stub.o')
def ours(op):
    gen = sh('python3', 'gen.py', op, str(N + 361)).stdout.split('\n')
    recs = [l for l in gen if l.strip().startswith('.dword')][361:]
    open(f'{T}/vec.inc', 'w').write('nrec = %d\nvec:\n%s\n' % (len(recs), '\n'.join(recs)))
    open(f'{T}/op.inc', 'w').write(f'.import {SYM[op]}\nOP = {SYM[op]}\n')
    assert sh('ca65', '-t', 'sim6502', '-U', '-I', T, *(['-DEXACT=1'] if op in CONV else []), f'{here}/drv.s', '-o', f'{T}/drv.o').returncode == 0
    os.makedirs(f'{T}/s', exist_ok=True)
    open(f'{T}/s/vec.inc', 'w').write('nrec = 1\nvec:\n .dword 0, 0, 0\n'); open(f'{T}/s/op.inc', 'w').write('.import stub_op\nOP = stub_op\n')
    assert sh('ca65', '-t', 'sim6502', '-U', '-I', f'{T}/s', *(['-DEXACT=1'] if op in CONV else []), f'{here}/drv.s', '-o', f'{T}/drv_s.o').returncode == 0
    lib = [f'{T}/sf32.o', f'{T}/sf32_muldiv.o', f'{T}/sf32_conv.o']
    for k, objs in (('s', [f'{T}/stub.o'] + lib), ('r', lib)):
        assert sh('ld65', '-C', 'sim6502_lut.cfg', '-m', f'{T}/{k}.map', f'{T}/drv_s.o' if k == 's' else f'{T}/drv.o', *objs, 'sim6502.lib', '-o', f'{T}/{k}.bin').returncode == 0
    def ramsegs(k):
        m = open(f'{T}/{k}.map').read().split('Segment list:')[1]
        return {l.split()[0]: l.split()[3] for l in m.splitlines() if l.split()[:1] in (['DATA'], ['BSS'])}
    assert ramsegs('s') == ramsegs('r'), 'base and library runs must link the same DATA and BSS: crt0 initializes them before main, and the difference lands in every delta'
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
    assert out['s'][0][1] == 0, 'the base run must pass its record: a failing one adds ~125 cycles of report loop to the base (2026-11-02: every 6502 mean/worst was 125 low)'
    b = out['s'][0][0]
    deltas = [c - b for c, _ in out['r']]; fails = sum(1 for _, f in out['r'] if f)
    return sum(deltas) / len(deltas), max(deltas), fails, recs
def mos(op, recs):
    e = {'add': 'x.f + y.f', 'sub': 'x.f - y.f', 'mul': 'x.f * y.f', 'div': 'x.f / y.f', 'itof': '(float)(int32_t)x.u', 'utof': '(float)x.u'}.get(op)
    v = [[int(t.strip().lstrip('$'), 16) for t in r.split('.dword')[1].split(',')] for r in recs]
    c = ['#include <stdint.h>', '#include <stdio.h>', 'typedef union { float f; uint32_t u; } U;']
    for n, k in (('A', 0), ('B', 1), ('W', 2)): c.append('static const uint32_t %s[%d] = {%s};' % (n, len(v), ','.join('0x%xu' % t[k] for t in v)))
    c.append('static volatile uint32_t *const clk = (volatile uint32_t *)0xFFF0;')
    c.append('int main(void) { unsigned long sum = 0, sb = 0; uint32_t mx = 0, mb = 0; unsigned fails = 0;')
    c.append(' for (unsigned i = 0; i < %d; i++) { U x, y, r, s; x.u = A[i]; y.u = B[i];' % len(v))
    c.append('  uint32_t t0 = *clk; s.u = x.u; uint32_t t1 = *clk; %s uint32_t t2 = *clk;' % ('r.f = ' + e + ';' if e else {'ftoi': 'r.u = (uint32_t)(int32_t)x.f;', 'ftou': 'r.u = (uint32_t)x.f;'}[op]))
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
