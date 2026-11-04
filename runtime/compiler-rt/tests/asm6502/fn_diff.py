#!/usr/bin/env python3
"""fn_diff.py A.s [B.s ...] -> per llrm-cc function (a `.segment "BSS"` line starts the next one): name, instruction count, and with two
sets of files which functions differ. Local labels are renamed by order, so only code differences show. A function is compared whole."""
import re, sys

def funcs(paths):
    d = {}
    for p in paths:
        for chunk in re.split(r'^\.segment "BSS"\n', open(p).read(), flags=re.M):
            code = chunk.split('.segment "CODE"\n', 1)[-1]
            labels = re.findall(r'^([A-Za-z_]\w*):', code, re.M)
            if not labels: continue
            ren = {l: 'L%d' % i for i, l in enumerate(labels)}
            body = [re.sub(r'[A-Za-z_]\w*', lambda m: ren.get(m.group(0), m.group(0)), l.strip()) for l in code.split('\n') if l.strip() and not l.startswith('.') and not re.match(r'\w+ = ', l)]
            d[labels[0]] = body
    return d

if __name__ == '__main__':
    args = sys.argv[1:]
    if '--' not in args: args += ['--']
    i = args.index('--'); a, b = funcs(args[:i]), funcs(args[i + 1:])
    for n in sorted(set(a) | set(b)):
        if n not in a or n not in b: print(n, 'only in', 'B' if n not in a else 'A')
        elif a[n] != b[n]: print(n, 'differs', len(a[n]), len(b[n]))
