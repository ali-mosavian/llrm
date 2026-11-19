import re, sys
import os
src = open(sys.argv[1]).read()
# The header is read from the repo, not from a copy beside the output: a copy every job rewrote could be read half written.
src = src.replace('"../bench_io.h"', '"%s"' % os.path.abspath(os.path.join(os.path.dirname(sys.argv[1]), '..', 'bench_io.h')))
src = 'static volatile int opq0;\n#define OPQ(v) ((__typeof__(v))((v) + opq0))\n' + src
src = re.sub(r'(?m)^((?:static\s+)?(?:unsigned\s+|signed\s+)?\w+(?:\s+\w+)?\s+)(bench_\w+\s*\()', r'__attribute__((noinline)) \1\2', src, count=0)
def wrap(m):
    name, args = m.group(1), m.group(2)
    if not args.strip() or args.strip() == 'void': return m.group(0)
    parts, depth, cur = [], 0, ''
    for ch in args:
        if ch == ',' and depth == 0: parts.append(cur); cur = ''; continue
        depth += ch in '([{'; depth -= ch in ')]}'; cur += ch
    parts.append(cur)
    return name + '(' + ', '.join(f'OPQ({a.strip()})' for a in parts) + ')'
i = src.index('int main')
src = src[:i] + re.sub(r'(bench_\w+)\(([^;]*?)\)(?=\s*\)|\s*;)', wrap, src[i:])
open(sys.argv[2], 'w').write(src)
