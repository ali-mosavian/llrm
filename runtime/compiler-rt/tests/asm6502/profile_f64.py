#!/usr/bin/env python3
"""profile_f64.py DIR [N] [OPS]: per-function cycles of the 6502 binary64 add/sub/mul/div that bench_f64.py built in DIR (the directory
it prints). Each of the N records (default 100) is one sim65 run with start_idx/count_n patched as the bench does; the run is
profiled with profile6502.py and its cycle total and exit code must equal sim65's own (a difference is a bug in the profiler: exit 1).
Env: BLOCKS=1 profiles per label instead of per function; ANNOTATE=name prints that function's listing with counts summed over the records.
Prints per op the top functions by exclusive cycles per record, and the opcode-class histogram."""
import os, subprocess, sys
import profile6502 as P

DIR = sys.argv[1] if len(sys.argv) > 1 else '.'; N = int(sys.argv[2]) if len(sys.argv) > 2 else 100
OPS = sys.argv[3].split() if len(sys.argv) > 3 else ['add', 'sub', 'mul', 'div']
OPT = os.environ.get('OPT', 'Os')
DRIVER = {'zerobss', 'startup', 'callmain', 'initlib', 'donelib', 'main', '_main', 'exit', 'pushax', 'push0', 'pusha0'}

def sh(*a):
    p = subprocess.run(a, capture_output=True, text=True)
    if p.returncode: raise SystemExit('%s\n%s%s' % (' '.join(a), p.stdout[-2000:], p.stderr[-2000:]))
    return p

def relink_with_debug(d):
    """The bench's link again with ca65 -g and --dbgfile (names of the unexported labels); the image must not change."""
    objs = []
    for src in [f'{d}/full.s'] + [f'{d}/r/{OPT}_{f}.s' for f in ('sf64', 'int64')] + [f'{DIR}/mos.s']:
        o = f'{d}/g_{os.path.basename(src)[:-2]}.o'; sh('ca65', '-g', '-t', 'sim6502', '-U', src, '-o', o); objs.append(o)
    sh('cl65', '-t', 'sim6502', '-o', f'{d}/g.bin', '-Ln', f'{d}/g.lbl', '-Wl', f'--dbgfile,{d}/g.dbg', *objs)
    assert open(f'{d}/g.bin', 'rb').read() == open(f'{d}/r.bin', 'rb').read(), 'the -g link differs from the bench image'

def patcher(d):
    lab = P.parse_labels(f'{d}/g.lbl'); data = open(f'{d}/g.bin', 'rb').read()
    os_, oc = 12 + lab['bench_start'] - 0x200, 12 + lab['bench_count'] - 0x200
    def patched(start, count):
        b = bytearray(data)
        assert b[os_] == 1 and b[os_ + 1] == 0 and b[oc] == 1 and b[oc + 1] == 0
        b[os_], b[os_ + 1], b[oc], b[oc + 1] = start & 255, start >> 8, count & 255, count >> 8
        return bytes(b)
    return patched, P.labels_for(f'{d}/g.lbl', f'{d}/g.dbg')

def sim65(image, path):
    open(path, 'wb').write(image)
    p = subprocess.run(['sim65', '-c', path], capture_output=True, text=True)
    return int(p.stdout.split()[0]), p.returncode

def main():
    total_bad = 0
    for op in OPS:
        bad = 0
        d = f'{DIR}/{OPT}_{op}'; relink_with_debug(d); patched, labels = patcher(d)
        runs = []
        for i in range(1, N + 1):
            img = patched(i, 1); r = P.run(img, labels, blocks=bool(os.environ.get('BLOCKS'))); c, rc = sim65(img, f'{d}/q.bin'); runs.append(r)
            if (r.cycles, r.exit) != (c, rc):
                bad += 1; print(f'MISMATCH {op} record {i}: profiler {r.cycles} exit {r.exit}, sim65 {c} exit {rc}')
        total_bad += bad; t = P.merge(runs)
        print(f'\n== {op} ({OPT}): {N} records, totals and exit codes equal to sim65 on {N - bad} of {N}; mean {t.cycles / N:.0f} cycles/record')
        lib = {a: v for a, v in t.funcs.items() if v[0] not in DRIVER}
        print('%-26s %8s %10s %10s %6s' % ('function (per record)', 'calls', 'excl', 'incl', 'excl%'))
        for a, (nm, ca, ex, inc) in sorted(lib.items(), key=lambda kv: -kv[1][2])[:12]:
            print('%-26s %8.1f %10.0f %10.0f %5.1f%%' % (nm, ca / N, ex / N, inc / N, 100 * ex / t.cycles))
        if os.environ.get('ANNOTATE'): P.annotate(t, labels, os.environ['ANNOTATE'])  # counts summed over the records
        print('driver+startup (excl, per record): %.0f' % (sum(v[2] for v in t.funcs.values() if v[0] in DRIVER) / N))
        hist = {c: [0, 0] for c in P.CLASSES}  # library code only: the startup's zero-fill is not the operation
        for a in lib:
            for c, (n, cy) in t.fclasses.get(a, {}).items(): hist[c][0] += n; hist[c][1] += cy
        tot = sum(v[1] for v in hist.values())
        print('class (library code)  insns/rec cycles/rec   cyc%')
        for c in P.CLASSES: print('%-8s %20.0f %9.0f %6.1f%%' % (c, hist[c][0] / N, hist[c][1] / N, 100 * hist[c][1] / tot))
    sys.exit(1 if total_bad else 0)
if __name__ == '__main__': main()
