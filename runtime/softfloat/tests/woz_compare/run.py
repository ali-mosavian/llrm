#!/usr/bin/env python3
"""Rankin/Woz 6502 float routines vs ours on sim65: bytes (reachable code) and cycles per call, same operand values.
Woz code is read from the listing at run time and never copied into the repo.
Usage: LLRM_CC=... run.py [N]   (env WOZ_LISTING, MOS_RT override the default paths)"""
import os, re, subprocess, sys, tempfile, struct

HERE = os.path.dirname(os.path.abspath(__file__)); SF = os.path.abspath(os.path.join(HERE, '..', '..'))
LISTING = os.environ.get('WOZ_LISTING', os.path.expanduser('~/scratch/hand-sources/site/floats/wozfp3.txt'))
MOS_RT = os.environ.get('MOS_RT', os.path.expanduser('~/scratch/handoff/mos_rt.s'))
CC = os.environ['LLRM_CC']; N = int(sys.argv[1]) if len(sys.argv) > 1 else 64
T = tempfile.mkdtemp(prefix='woz_', dir=os.environ.get('TMPDIR'))
OPS = ['add', 'sub', 'mul', 'div', 'i2f', 'f2i']
WOZ_ENTRY = {'add': 0xF46E, 'sub': 0xF468, 'mul': 0xF48C, 'div': 0xF4B2, 'i2f': 0xF451, 'f2i': 0xF640}
FAST_FN = {'add': 'fz_add', 'sub': 'fz_sub', 'mul': 'fz_mul', 'div': 'fz_div', 'i2f': 'fz_from_i16', 'f2i': 'fz_to_i16'}
OURS_FN = {'add': '__addsf3', 'sub': '__subsf3', 'mul': '__mulsf3', 'div': '__divsf3', 'i2f': '__floatsisf', 'f2i': '__fixsfsi'}

def sh(*a, **k):
    r = subprocess.run(a, capture_output=True, text=True, **k)
    if r.returncode and not k.get('ok'):
        sys.exit('FAILED: %s\n%s%s' % (' '.join(a), r.stdout[-500:], r.stderr[-800:]))
    return r

def cycles(binfile):
    r = subprocess.run(['sim65', '-c', '-x', '2000000000', binfile], capture_output=True, text=True)
    m = re.search(r'(\d+) cycles', r.stdout + r.stderr)
    return int(m.group(1)), r.returncode, r.stdout

# ---- Woz listing -> instructions
ins = {}
for ln in open(LISTING):
    m = re.match(r'^([0-9A-F]{4}): ((?:[0-9A-F]{2} ?)+)', ln)
    if m:
        ins[int(m.group(1), 16)] = [int(x, 16) for x in m.group(2).split()]
def reach(entry):
    seen, work = set(), [entry]
    while work:
        a = work.pop()
        if a in seen or a not in ins: continue
        seen.add(a); b = ins[a]; op = b[0]; nxt = a + len(b)
        if op == 0x20: work += [b[1] | b[2] << 8, nxt]
        elif op == 0x4C: work.append(b[1] | b[2] << 8)
        elif op == 0x60: pass
        elif op & 0x1F == 0x10: work += [nxt + (b[1] - 256 if b[1] > 127 else b[1]), nxt]
        else: work.append(nxt)
    return sum(len(ins[a]) for a in seen)
woz_bytes = {op: reach(a) for op, a in WOZ_ENTRY.items()}

# ---- Woz link
secs = {}
for a in sorted(ins):
    k = 'WOZA' if a < 0xF600 else 'WOZB'
    secs.setdefault(k, []).append((a, ins[a]))
with open(f'{T}/woz_code.s', 'w') as f:
    for k, lst in secs.items():
        f.write('.segment "%s"\n' % k)
        for a, b in lst: f.write('.byte ' + ','.join(str(x) for x in b) + '\n')
cfg = open('/usr/share/cc65/cfg/sim6502.cfg').read()
cfg = cfg.replace('size = $FDF0 - __STACKSIZE__', 'size = $F000 - $0200 - __STACKSIZE__')
cfg = cfg.replace('MEMORY {', 'MEMORY {\n    WA: file = "", start = $F425, size = $D7;\n    WB: file = "", start = $F63D, size = $21;', 1)
cfg = cfg.replace('SEGMENTS {', 'SEGMENTS {\n    WOZA: load = MAIN, run = WA, type = rw, define = yes;\n    WOZB: load = MAIN, run = WB, type = rw, define = yes;', 1)
open(f'{T}/woz.cfg', 'w').write(cfg)
sh(sys.executable, f'{HERE}/gen.py', T, str(N))
res = {}
for op, i in zip(OPS, range(6)):
    open(f'{T}/woz_equ.inc', 'w').write('SIGN=$F3\nX2=$F4\nM2=$F5\nX1=$F8\nM1=$F9\nfpregs=$F4\nNREC=%d\ntarget=$%04X\n' % (N, WOZ_ENTRY[op]))
    sh('sh', '-c', f'cp {T}/woz_{op}.inc {T}/woz_tab.inc')
    out = {}
    for mode, defs in (('call', []), ('nocall', ['-D', 'NOCALL']), ('dump', ['-D', 'DUMP'])):
        sh('ca65', '-t', 'sim6502', *defs, '-I', T, '-o', f'{T}/d_{mode}.o', f'{HERE}/woz_driver.s')
        sh('ca65', '-t', 'sim6502', '-o', f'{T}/code.o', f'{T}/woz_code.s')
        sh('ld65', '-C', f'{T}/woz.cfg', '-o', f'{T}/w_{mode}.bin', f'{T}/d_{mode}.o', f'{T}/code.o', '/usr/share/cc65/lib/sim6502.lib')
        if mode != 'dump': out[mode] = cycles(f'{T}/w_{mode}.bin')
    # decode the dumped results and compare with the exact value (instrument check)
    raw = subprocess.run(['sim65', '-x', '2000000000', f'{T}/w_dump.bin'], capture_output=True).stdout
    exp = [float(x) for x in open(f'{T}/expect_{op}.txt')]
    worst = 0.0
    for k in range(N):
        e, m1, m2, m3 = raw[4 * k:4 * k + 4]
        if op == 'f2i':
            got = float(struct.unpack('>h', bytes([m1, m2]))[0])
        else:
            m = (m1 << 16 | m2 << 8 | m3); m = m - (1 << 24) if m >> 23 else m
            got = m / 2 ** 22 * 2.0 ** (e - 128)
        w = exp[k]
        worst = max(worst, abs(got - w) / max(abs(w), 1e-30) if op != 'f2i' else abs(got - w))
    res[op] = {'woz_cyc': (out['call'][0] - out['nocall'][0]) / N, 'woz_err': worst, 'woz_len': len(raw)}

# ---- ours
rtlines = open(MOS_RT).read()
for op in OPS:
    h = open(f'{T}/ours_{op}.h').read(); open(f'{T}/tables.h', 'w').write(h)
    asm = {}
    for f in ('sf32', 'sf32_conv'):
        sh(CC, '--target=6502-sim65', '-Os', '-S', '-DSF_INTABI', f'-I{SF}/include', f'{SF}/src/{f}.c', '-o', f'{T}/{f}.s')
    cyc = {}
    for mode, d in (('call', [f'-DOP={OPS.index(op)}']), ('nocall', ['-DNOCALL'])):
        sh(CC, '--target=6502-sim65', '-Os', '-S', *d, f'-I{T}', f'{HERE}/ours_main.c', '-o', f'{T}/m.s')
        with open(f'{T}/p.s', 'w') as f:
            f.write(''.join(l for l in open(f'{T}/m.s') if re.match(r'^rc\d+ = ', l)))
            f.write('.export _main\n_main:\n  jsr main\n  rts\n')
            for n in ('m', 'sf32', 'sf32_conv'):
                f.write(''.join(l for l in open(f'{T}/{n}.s') if not re.match(r'^rc\d+ = ', l)))
            f.write(rtlines)
        sh('cl65', '-t', 'sim6502', '-o', f'{T}/o.bin', f'{T}/p.s')
        c, rc, _ = cycles(f'{T}/o.bin'); cyc[mode] = c
        if mode == 'call' and rc: res[op]['ours_wrong'] = rc
    res[op]['ours_cyc'] = (cyc['call'] - cyc['nocall']) / N

# ---- spike fast32 (same Woz word layout), compiled by llrm-cc
for op in OPS:
    open(f'{T}/tables.h', 'w').write(open(f'{T}/fast_{op}.h').read())
    w = sh('cc', '-O2', f'-DOP={OPS.index(op)}', f'-I{T}', f'{HERE}/fast_expect.c', f'{SF}/src/fast32.c', '-o', f'{T}/fe').returncode
    wtab = sh(f'{T}/fe').stdout
    open(f'{T}/tables.h', 'a').write(wtab)
    sh(CC, '--target=6502-sim65', '-Os', '-S', f'-I{SF}/include', f'{SF}/src/fast32.c', '-o', f'{T}/fast32.s')
    cyc = {}
    for mode, d in (('call', [f'-DOP={OPS.index(op)}']), ('nocall', ['-DNOCALL'])):
        sh(CC, '--target=6502-sim65', '-Os', '-S', *d, f'-I{T}', f'{HERE}/fast_main.c', '-o', f'{T}/m.s')
        with open(f'{T}/p.s', 'w') as f:
            f.write(''.join(l for l in open(f'{T}/m.s') if re.match(r'^rc\d+ = ', l)))
            f.write('.export _main\n_main:\n  jsr main\n  rts\n')
            for n in ('m', 'fast32'):
                f.write(''.join(l for l in open(f'{T}/{n}.s') if not re.match(r'^rc\d+ = ', l)))
            f.write(rtlines)
        sh('cl65', '-t', 'sim6502', '-o', f'{T}/o.bin', f'{T}/p.s')
        c, rc, _ = cycles(f'{T}/o.bin'); cyc[mode] = c
        if mode == 'call' and rc: res[op]['fast_wrong'] = rc
    res[op]['fast_cyc'] = (cyc['call'] - cyc['nocall']) / N

# ---- ours: bytes reachable from the entry (call graph over jsr, sizes from the ca65 listing)
def listing(path, hdr):
    sh('ca65', '-U', '-l', path + '.lst', path, '-o', '/dev/null')
    return open(path + '.lst').read().split('\n')
sizes = {}; calls = {}; defined = set()
for src in (f'{T}/sf32.s', f'{T}/sf32_conv.s', f'{T}/fast32.s', f'{T}/rtfull.s'):
    if src.endswith('rtfull.s'):
        with open(src, 'w') as f:
            f.write(''.join(l for l in open(f'{T}/sf32.s') if re.match(r'^rc\d+ = ', l))); f.write(rtlines)
    L = listing(src, None); labs = []
    for ln in L:
        m = re.match(r'^([0-9A-F]{6})r\s+\d+\s+(.*)$', ln)
        if not m: continue
        off = int(m.group(1), 16); rest = m.group(2)
        lm = re.match(r'^([A-Za-z_]\w*):', rest) if rest and not rest[0].isspace() else None
        if lm: labs.append([lm.group(1), off, []])
        jm = re.search(r'\bjsr\s+(\w+)', rest)
        if jm and labs: labs[-1][2].append(jm.group(1))
    end = max([int(re.match(r'^([0-9A-F]{6})r', l).group(1), 16) + len(re.findall(r' [0-9A-F]{2}(?= |$)', l[14:26])) for l in L if re.match(r'^[0-9A-F]{6}r\s+\d+\s+[0-9A-F]{2}', l)] + [0])
    names = [l[0] for l in labs]
    top = set(n for n in names if not any(n != m and n.startswith(m + '_') for m in names))
    cur = None
    for k, (n, off, cl) in enumerate(labs):
        nxt = labs[k + 1][1] if k + 1 < len(labs) else end
        if n in top: cur = n; sizes[cur] = 0; calls[cur] = set(); defined.add(cur)
        if cur: sizes[cur] += max(0, nxt - off); calls[cur] |= set(cl)
def closure(fn):
    seen, work = set(), [fn]
    while work:
        f = work.pop()
        if f in seen or f not in sizes: continue
        seen.add(f); work += list(calls[f])
    return sum(sizes[f] for f in seen), sorted(seen)
for op in OPS:
    res[op]['ours_bytes'], res[op]['ours_fns'] = closure(OURS_FN[op])
    res[op]['fast_bytes'], _ = closure(FAST_FN[op])
    res[op]['woz_bytes'] = woz_bytes[op]

print('%-4s %7s %8s %7s %9s %9s %8s  %s' % ('op', 'Woz B', 'fast B', 'ieee B', 'Woz cyc', 'fast cyc', 'ieee cyc', 'Woz max err (rel; abs for f2i)'))
for op in OPS:
    r = res[op]
    print('%-4s %7d %8d %7d %9.0f %9.0f %8.0f  %.2e%s%s' % (op, r['woz_bytes'], r['fast_bytes'], r['ours_bytes'], r['woz_cyc'], r['fast_cyc'], r['ours_cyc'], r['woz_err'], '  IEEE WRONG' if 'ours_wrong' in r else '', '  FAST WRONG' if 'fast_wrong' in r else ''))
print('N=%d  workdir %s' % (N, T))
