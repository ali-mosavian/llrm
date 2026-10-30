#!/usr/bin/env python3
"""Differential check of simplify.rs against `opt -passes=instsimplify`: `tools/simplify_diff.py N SEED [WORKDIR]`.

Generates N one-operation functions (simplify_cases.py), asks LLVM what it makes of `%t`, asks `simplify_cases`
(simplify_tests.rs), and prints the cases where the answers differ: `WRONG` (we simplify and LLVM does not, or to
another value) and `GAP` (LLVM simplifies, we do not). Exit 1 on any WRONG."""
import os, re, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
OPT = os.path.expanduser('~/scratch/llvm-build/bin/opt')
n, seed = sys.argv[1], sys.argv[2]
work = sys.argv[3] if len(sys.argv) > 3 else tempfile.mkdtemp()
src = os.path.join(work, 'cases.ll')
subprocess.run([sys.executable, os.path.join(HERE, 'simplify_cases.py'), n, seed, src], check=True)
llvm = subprocess.run([OPT, '-passes=instsimplify', '-S', src], capture_output=True, text=True, check=True).stdout
out = os.path.join(work, 'ours.txt')
subprocess.run(['cargo', 'test', '--release', '-q', '-p', 'llrm-mir', '--lib', 'simplify_cases', '--', '--ignored'],
               env={**os.environ, 'LLRM_SIMPLIFY_CASES': src, 'LLRM_SIMPLIFY_OUT': out}, check=True, capture_output=True)
ours = dict(l.split(' ', 1) for l in open(out).read().splitlines())
def norm(v, ty):
    w = int(ty[1:])
    if v == 'true': return '1'
    if v == 'false': return '0'
    if v == 'poison': return v
    if re.fullmatch(r'-?\d+', v): return str(int(v) & ((1 << w) - 1))
    return v
cases = {}
for body in re.split(r'(?m)^(?=define )', open(src).read())[1:]:
    cases[re.match(r'define void @(\w+)', body).group(1)] = body
theirs = {}
skipped = set()
for body in re.split(r'(?m)^(?=define )', llvm)[1:]:
    name = re.match(r'define void @(\w+)', body).group(1)
    stores = [l for l in body.splitlines() if 'store volatile' in l and ('@g_i' in l)]
    m = re.search(r'store volatile (i\d+) (\S+), ptr @g_i\d+', stores[-1])
    ty, val = m.group(1), m.group(2).rstrip(',')
    # opt simplifies to a fixed point: a case whose other instructions it
    # changed first measures the pass, not the one query, and is skipped.
    kept = set(re.findall(r'(?m)^\s+(%\w+) = ', body))
    wanted = set(re.findall(r'(?m)^\s+(%\w+) = ', cases[name]))
    skipped_names = wanted - kept - {'%t'}
    if skipped_names:
        skipped.add(name)
    theirs[name] = None if val == '%t' else norm(val, ty)
wrong = gap = same = 0
for name, body in cases.items():
    if name in skipped:
        continue
    mine = ours[name].strip()
    mine = None if mine == 'none' else mine.split(' ', 1)[1]
    ty = re.search(r'store volatile (i\d+) %t', body).group(1)
    mine = norm(mine, ty) if mine is not None else None
    t = theirs[name]
    if mine == t: same += 1; continue
    kind = 'GAP' if mine is None else 'WRONG'
    gap += kind == 'GAP'; wrong += kind == 'WRONG'
    print(kind, f'ours={mine} llvm={t}', [l.strip() for l in body.splitlines() if re.match(r'\s+%t = ', l) or (l.startswith('  %') and '%t' not in l and 'store' not in l)], sep=' | ')
print(f'llvm_simplified={sum(v is not None for v in theirs.values())} ', end='')
print(f'skipped={len(skipped)} ', end='')
print(f'cases={len(cases)} same={same} gap={gap} wrong={wrong}')
sys.exit(1 if wrong else 0)
