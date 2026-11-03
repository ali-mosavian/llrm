#!/usr/bin/env python3
"""bench_f64.py [N] [OPS] [OPTS]: mean and worst cycles per call and code bytes of the 6502 binary64 add/sub/mul/div, ours (lib/fp/f64 and
lib/int/int64.c through $LLRM_CC at each of OPTS, default "Os O2", on sim65: one run per record with the driver's start index and count patched
into the binary) and llvm-mos (compiler-rt soft double, mos-sim, the clock read around each call). The N operand pairs (default 300)
are the first N of tests/gen_vectors64.c's stream: random bits, denormals, edge values, one pair in eight special. Cycles are the call
minus a stub that returns zero (A, X, rc2..rc7: eight stores, about 20 cycles, and a base record that must pass) for ours, minus a copy of the operand for llvm-mos. Bytes are the code reachable
from the entry point: the library functions it calls (sim65_reach.py) plus the mos_rt.s routines they use; llvm-mos: text of the program
with the operation minus the program without it. SDCC and cc65 have a 32-bit double: no f64 column for them."""
import os, re, subprocess, sys, tempfile
here = os.path.dirname(os.path.abspath(__file__)); SF = os.path.join(here, '../..'); TESTS = os.path.join(here, '..')
N = int(sys.argv[1]) if len(sys.argv) > 1 else 300
OPS = sys.argv[2].split() if len(sys.argv) > 2 and sys.argv[2] else ['add', 'sub', 'mul', 'div']
OPTS = sys.argv[3].split() if len(sys.argv) > 3 else ['Os', 'O2']
CC = os.environ['LLRM_CC']
MOS_RT = os.environ['MOS_RT']  # mos_rt.s, as for run_sim65_f64.sh
MOS = os.environ['LLVM_MOS'] + '/bin'  # the llvm-mos SDK directory
T = tempfile.mkdtemp()
SYM = {'add': '__adddf3', 'sub': '__subdf3', 'mul': '__muldf3', 'div': '__divdf3'}
COL = {'add': 4, 'sub': 6, 'mul': 8, 'div': 10}  # the result's hi word in gen_vectors64.c's record
def sh(*a, **k):
    p = subprocess.run(a, cwd=k.get('cwd', SF), capture_output=True, text=True)
    if k.get('check', True) and p.returncode: raise SystemExit('%s\n%s%s' % (' '.join(a), p.stdout[-2000:], p.stderr[-2000:]))
    return p

sh('cc', '-O2', '-ffp-contract=off', f'{TESTS}/gen_vectors64.c', '-o', f'{T}/gen')
recs = [[int(t.rstrip('u'), 0) & 0xFFFFFFFF for t in l.strip().rstrip(',').strip('{}').split(',')] for l in sh(f'{T}/gen', str(N)).stdout.split('\n') if l.startswith('  {')]
assert len(recs) == N and all(len(r) == 16 for r in recs)
# mos_rt.s with the zero-page equates and its __sa/__sb cells exported (the compiler's 32-bit helper protocol)
open(f'{T}/mos.s', 'w').write(''.join('rc%d = %d\n' % (i, 128 + i) for i in range(26)) + open(MOS_RT).read() + '\n.export __sa, __sb\n')
sh('ca65', '-t', 'sim6502', f'{T}/mos.s', '-o', f'{T}/mos.o')
WRAP = '''.export _main
.bss
sstk: .res 256
.code
_main:
  lda #<(sstk+256)
  sta rc0
  lda #>(sstk+256)
  sta rc1
  jsr main
  rts
'''
_srcs = subprocess.check_output([f'{here}/../fp_sources.sh', 'f64'], text=True).split() + ['lib/int/int64.c']
LIBPATH = {os.path.dirname(p).split('/')[-1] + '_' + os.path.basename(p)[:-2]: p for p in _srcs}
LIBS = tuple(LIBPATH)
def compile_libs(opt):
    for f in LIBS: sh(CC, '--target=6502-sim65', f'-{opt}', '-S', '-DSF_INTABI', *os.environ.get('SF_EXTRA', '').split(), '-Iinclude', LIBPATH[f], '-o', f'{T}/{opt}_{f}.s')

def strip_exports(path):  # llrm-cc exports statics, which collide across objects
    s = open(path).read()
    open(path, 'w').write(re.sub(r'^\.export (?!__|sf32_pack)\S+\n', '', s, flags=re.M))

def code_size(obj):
    return int(re.search(r'CODE:\s+(\d+)', sh('od65', '--dump-segsize', obj).stdout).group(1))

def mos_rt_bytes(libs_s, labels, mapfile):
    """Bytes of the mos_rt.s routines the reduced library calls, with the routines they call in turn (label to next label)."""
    text = open(MOS_RT).read().split('\n'); lab = {}
    for i, l in enumerate(text):
        m = re.match(r'^([A-Za-z_]\w*):', l)
        if m: lab[m.group(1)] = i
    addr = {}
    for l in open(labels):
        m = re.match(r'al ([0-9A-F]+) \.(\w+)', l)
        if m: addr[m.group(2)] = int(m.group(1), 16)
    mm = re.search(r'mos\.o:\s+CODE\s+Offs=\w+\s+Size=(\w+)\s+Align=\w+\s+Fill=\w+', open(mapfile).read())
    start = min(addr[n] for n in lab if n in addr)
    end = start + int(mm.group(1), 16)
    top = sorted(a for n, a in addr.items() if n in lab and a >= start)  # a routine ends at the next label of mos_rt.s
    def size(n):
        a = addr[n]; nxt = [b for b in top if b > a]
        return (nxt[0] if nxt else end) - a
    todo = set(re.findall(r'^\s+(?:jsr|jmp)\s+(\w+)', ''.join(open(p).read() for p in libs_s), flags=re.M)) & set(lab)
    seen = set()
    while todo:
        n = todo.pop(); seen.add(n)
        body = []
        for l in text[lab[n] + 1:]:
            if re.match(r'^[A-Za-z_]\w*:', l): break
            body.append(l)
        for c in re.findall(r'^\s+(?:jsr|jmp)\s+(\w+)', '\n'.join(body), flags=re.M):
            if c in lab and c not in seen: todo.add(c)
    return sum(size(n) for n in seen if n in addr), sorted(seen)

def ours(op, opt):
    vec = '#include <stdint.h>\nstatic const uint32_t V[%d][6] = {\n  {0,0,0,0,0,0},\n%s};\n' % (
        N + 1, ''.join('  {%s},\n' % ','.join('0x%08Xu' % x for x in (r[0], r[1], r[2], r[3], r[COL[op]], r[COL[op] + 1])) for r in recs))
    d = f'{T}/{opt}_{op}'; os.makedirs(d, exist_ok=True)
    open(f'{d}/bench_vec.h', 'w').write(vec)
    sh(CC, '--target=6502-sim65', '-O2', '-S', '-DSF_INTABI', f'-DOP={SYM[op]}', f'-I{d}', f'{here}/bench_main64.c', '-o', f'{d}/main.s')
    open(f'{d}/full.s', 'w').write(open(f'{d}/main.s').read() + WRAP + '.export bench_start = start_idx\n.export bench_count = count_n\n')
    sh('ca65', '-t', 'sim6502', '-U', f'{d}/full.s', '-o', f'{d}/main.o')
    sh('python3', f'{here}/../sim65_reach.py', f'{d}/r', 'main', f'{d}/main.s', *[f'{T}/{opt}_{f}.s' for f in LIBS])
    objs = []
    for f in LIBS:
        p = f'{d}/r/{opt}_{f}.s'; strip_exports(p)
        sh('ca65', '-t', 'sim6502', '-U', p, '-o', p[:-2] + '.o'); objs.append(p[:-2] + '.o')
    for a in os.environ.get('SF_ASM', '').split():  # extra hand-written library sources (e.g. config/6502/di64.s), linked into the library build only
        r_ = sh('ca65', '-t', 'sim6502', '-I', f'{here}/../../config/6502', a, '-o', f'{d}/{os.path.basename(a)}.o'); assert r_.returncode == 0, r_.stderr; objs.append(f'{d}/{os.path.basename(a)}.o')
    open(f'{d}/stub.s', 'w').write('rc2 = 130\n' + ''.join('rc%d = %d\n' % (i, 128 + i) for i in range(3, 8)) + f'.export {SYM[op]}\n{SYM[op]}:\n lda #0\n ldx #0\n' + ''.join(f' sta rc{i}\n' for i in range(2, 8)) + ' rts\n')
    bss = sum(int(m.group(1)) for o in objs for m in [re.search(r'BSS:\s+(\d+)', sh('od65', '--dump-segsize', o).stdout)] if m)
    if bss: open(f'{d}/stub.s', 'a').write(f'.segment "BSS"\n .res {bss}\n')  # same BSS as the library build: crt0's zerobss loop is not the operation
    sh('ca65', '-t', 'sim6502', f'{d}/stub.s', '-o', f'{d}/stub.o')
    for k, libo in (('r', objs), ('s', [f'{d}/stub.o'])):
        sh('cl65', '-t', 'sim6502', '-o', f'{d}/{k}.bin', '-Ln', f'{d}/{k}.lbl', '-m', f'{d}/{k}.map', f'{d}/main.o', *libo, f'{T}/mos.o')
    seg = lambda m: {a: int(b, 16) for a, b in re.findall(r'^(ZEROPAGE|BSS|DATA)\s+\w+\s+\w+\s+(\w+)', open(m).read(), re.M)}
    assert seg(f'{d}/r.map')['BSS'] == seg(f'{d}/s.map')['BSS'], f'{op} {opt}: the base binary must have the library build\'s BSS: crt0 zeroes it, and the extra cycles are not the operation'
    labels = {}
    for l in open(f'{d}/r.lbl'):
        m = re.match(r'al ([0-9A-F]+) \.(\w+)', l)
        if m: labels[m.group(2)] = int(m.group(1), 16)
    pat = {}
    for k in 'rs':
        data = bytearray(open(f'{d}/{k}.bin', 'rb').read())
        l = {}
        for ln in open(f'{d}/{k}.lbl'):
            m = re.match(r'al ([0-9A-F]+) \.(\w+)', ln)
            if m: l[m.group(2)] = int(m.group(1), 16)
        pat[k] = (data, 12 + l['bench_start'] - 0x200, 12 + l['bench_count'] - 0x200)
    def run(k, start, count):
        data, os_, oc = pat[k]; b = bytearray(data)
        assert b[os_] == 1 and b[os_ + 1] == 0 and b[oc] == 1 and b[oc + 1] == 0, 'patch offsets do not hit start_idx/count_n'
        b[os_], b[os_ + 1], b[oc], b[oc + 1] = start & 255, start >> 8, count & 255, count >> 8
        open(f'{d}/p.bin', 'wb').write(b)
        p = sh('sim65', '-c', f'{d}/p.bin', check=False)
        if not p.stdout.split(): raise SystemExit('sim65 rc=%d out=%r err=%r' % (p.returncode, p.stdout[:200], p.stderr[:200]))
        return int(p.stdout.split()[0]), p.returncode
    s0, rc = run('r', 1, 0)  # startup and an empty loop
    base, rc = run('s', 0, 1)
    assert rc == 0, 'the base run must pass its record: a failing one adds report-loop cycles to the base'
    deltas, fails = [], 0
    for i in range(1, N + 1):
        c, rc = run('r', i, 1); deltas.append(c - base); fails += rc != 0
    allc, rc = run('r', 1, N)  # one run over all records must cost what the single-record runs add up to
    assert rc == 0 and allc == sum(d + base for d in deltas) - (N - 1) * s0, f'{op} {opt}: a {N}-record run costs {allc}, the single runs imply {sum(d + base for d in deltas) - (N - 1) * s0}'
    assert fails == 0, f'{op} {opt}: {fails} of {N} records fail'
    assert min(deltas) > 0
    lib = sum(code_size(o) for o in objs)
    mrt, names = mos_rt_bytes([f'{d}/r/{opt}_{f}.s' for f in LIBS], f'{d}/r.lbl', f'{d}/r.map')
    return sum(deltas) / N, max(deltas), lib + mrt, base, names

def mos(op):
    e = {'add': 'x.d + y.d', 'sub': 'x.d - y.d', 'mul': 'x.d * y.d', 'div': 'x.d / y.d'}[op]
    def prog(noop):
        c = ['#include <stdint.h>', '#include <stdio.h>', 'typedef union { double d; uint64_t u; } U;']
        for n, k in (('A', 0), ('B', 2), ('W', COL[op])):
            c.append('static const uint64_t %s[%d] = {%s};' % (n, N, ','.join('0x%x%08xull' % (r[k], r[k + 1]) for r in recs)))
        c.append('static volatile uint32_t *const clk = (volatile uint32_t *)0xFFF0;')
        c.append('int main(void) { unsigned long sum = 0, sb = 0; uint32_t mx = 0, mb = 0; unsigned fails = 0;')
        c.append(' for (unsigned i = 0; i < %d; i++) { U x, y, r, s; x.u = A[i]; y.u = B[i];' % N)
        c.append('  uint32_t t0 = *clk; s.u = x.u; uint32_t t1 = *clk; r.d = %s; uint32_t t2 = *clk;' % ('x.d' if noop else e))
        c.append('  uint32_t db = t1 - t0, d = t2 - t1; sum += d; sb += db; if (d > mx) mx = d; if (db > mb) mb = db;')
        c.append('  if (r.u != W[i] && !(((r.u << 1) > 0xFFE0000000000000ull) && ((W[i] << 1) > 0xFFE0000000000000ull))) fails++; (void)s; }')
        c.append(' printf("%lu %lu %lu %lu %u\\n", sum, sb, (unsigned long)mx, (unsigned long)mb, fails); return 0; }')
        return '\n'.join(c)
    open(f'{T}/m0_{op}.c', 'w').write(prog(0))
    sh(f'{MOS}/mos-sim-clang', '-Os', '-o', f'{T}/m0_{op}', f'{T}/m0_{op}.c')
    out = []  # size: one call on volatile operands against none, independent of N
    for noop in (0, 1):
        open(f'{T}/z{noop}.c', 'w').write('#include <stdint.h>\ntypedef union { double d; uint64_t u; } U;\nvolatile U x, y, r;\nint main(void) { U a = x, b = y, c; c.d = %s; r = c; return 0; }\n' % ('a.d' if noop else e.replace('x.d', 'a.d').replace('y.d', 'b.d')))
        sh(f'{MOS}/mos-sim-clang', '-Os', '-o', f'{T}/z{noop}', f'{T}/z{noop}.c')
        out.append(int(sh(f'{MOS}/llvm-size', f'{T}/z{noop}.elf').stdout.split('\n')[1].split()[0]))
    s, sb, mx, mb, f = map(int, sh(f'{MOS}/mos-sim', f'{T}/m0_{op}').stdout.split())
    assert f == 0, f'llvm-mos {op}: {f} wrong results'
    return (s - sb) / N, mx - mb, out[0] - out[1]

print('llrm-cc md5', subprocess.run(['md5sum', CC], capture_output=True, text=True).stdout[:12], f'N={N} dir={T}')
print('op   | ' + ' | '.join(f'ours -{o} mean/worst clk, B' for o in OPTS) + ' | llvm-mos mean/worst clk, B')
for opt in OPTS: compile_libs(opt)
for op in OPS:
    cols = []
    for opt in OPTS:
        m, w, b, base, names = ours(op, opt)
        cols.append(f'{m:.0f} / {w}, {b} (base {base}, mos_rt {",".join(names)})')
    a, b, sz = mos(op)
    print(f'{op} | ' + ' | '.join(cols) + f' | {a:.0f} / {b}, {sz}', flush=True)
