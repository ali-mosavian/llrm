#!/usr/bin/env python3
"""fngap.py PROG LEVEL [DIST]: per-function .text size, ours vs gcc, 68000."""
import os, subprocess, sys, tempfile
p, lv = sys.argv[1:3]; D = os.path.expanduser('~/scratch/mfbuild/' + (sys.argv[3] if len(sys.argv) > 3 else 'five_new'))
CPU = os.environ.get('CPU','68000')
GR = os.path.expanduser('~/scratch/gcc-m68k/root'); G = GR + '/usr/bin/m68k-linux-gnu-'; R = os.path.expanduser('~/scratch/wt/stackfix')
env = dict(os.environ, LD_LIBRARY_PATH=GR + '/usr/lib/x86_64-linux-gnu', LLRM_CC_TARGET='m68k-'+CPU)
src = next(f for f in (f'{R}/tests/run/c/{p}.c', f'{R}/bench/{p}/{p}.c') if os.path.exists(f))
def sizes(o):
    out = subprocess.run([G + 'nm', '-S', o], env=env, capture_output=True, text=True).stdout
    return {l.split()[3]: int(l.split()[1], 16) for l in out.splitlines() if len(l.split()) == 4 and l.split()[2] in 'tT'}
with tempfile.TemporaryDirectory() as t:
    subprocess.run([G + 'gcc', '-m'+CPU, '-msoft-float', '-ffreestanding', f'-{lv}', '-w', '-c', '-o', t + '/g.o', src, '-I', R + '/bench', '-I', R + '/runtime/shared/posix', '-I', R + '/tests/run/c'], env=env, check=True)
    subprocess.run([D + '/llrm-cc', f'-{lv}', '-w', '-c', '-o', t + '/o.o', src, '-I', R + '/bench', '-I', R + '/runtime/shared/posix', '-I', R + '/tests/run/c'], env=env, check=True)
    o, g = sizes(t + '/o.o'), sizes(t + '/g.o')
rows = sorted(((o.get(k, 0) - g.get(k, 0), k, o.get(k), g.get(k)) for k in set(o) | set(g)), reverse=True)
print('total', sum(o.values()), sum(g.values()))
for r in rows[:int(os.environ.get('N', 10))]: print(*r)
