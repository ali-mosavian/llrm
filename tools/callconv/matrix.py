#!/usr/bin/env python3
"""matrix.py SEED N OUTDIR: a callee unit and a caller unit of N random signatures, and what a host compiler prints for them.

Scalars of every width, floats, near and far pointers, structs of 4, 8 and 12 bytes; some arguments go through an identity routine, some
parameters have their address taken. Each is called directly and through a pointer and its result reported. `host.c` is both units for
the host compiler, `index.txt` names the call of each output line. tests/run/c/regparm3_matrix_* are seeds of it."""
import random, sys, subprocess, pathlib
seed, n, out = int(sys.argv[1]), int(sys.argv[2]), pathlib.Path(sys.argv[3])
rng = random.Random(seed)
SCALARS = {  # name: (c type, is_float)
    'i8': 'signed char', 'u8': 'unsigned char', 'i16': 'short', 'u16': 'unsigned short', 'i32': 'long', 'u32': 'unsigned long',
    'i64': 'long long', 'f32': 'float', 'f64': 'double', 'np': 'short *', 'fp': 'short __far *',
    's4': 'struct S4', 's8': 'struct S8', 's12': 'struct S12'}
ARG = ['i8', 'u8', 'i16', 'u16', 'i32', 'u32', 'i64', 'f32', 'f64', 'np', 'fp', 's4', 's8', 's12']
RET = ['void', 'i8', 'u8', 'i16', 'u16', 'i32', 'u32', 'i64', 'f32', 'f64', 'np', 'fp', 's4', 's8', 's12']
HEAD = '''typedef signed char i8_t;
struct S4 { short a, b; };
struct S8 { long a; short b, c; };
struct S12 { long a, b, c; };
extern void report(long value);
extern short cells[8];
'''
def value(kind, k):
    """a C expression of `kind` for call number k (small, exactly representable)."""
    v = (k * 7 + 3) % 100
    return {'i8': f'(signed char){v - 50}', 'u8': f'(unsigned char){v + 100}', 'i16': f'(short){v * 300 - 9000}', 'u16': f'(unsigned short){v * 600}',
            'i32': f'{v * 70001 - 3000000}L', 'u32': f'{v * 99991 + 1000000}UL', 'i64': f'(long long){v * 1000003 - 40000000}LL * 1000LL',
            'f32': f'{v + 0.5}f', 'f64': f'{v * 3 + 0.25}', 'np': f'&cells[{v % 8}]', 'fp': f'(short __far *)&cells[{(v + 3) % 8}]',
            's4': f'mk4({v}, {v + 1})', 's8': f'mk8({v * 1000}L, {v}, {v + 2})', 's12': f'mk12({v * 10}L, {v + 5}L, {v * 7}L)'}[kind]
def fold(kind, e):
    """an expression of long for argument expression `e` of `kind`."""
    return {'np': f'(long)*{e}', 'fp': f'(long)*{e}', 's4': f'({e}.a * 3L + {e}.b)', 's8': f'({e}.a + {e}.b * 5L + {e}.c)', 's12': f'({e}.a ^ {e}.b ^ {e}.c)',
            'f32': f'(long)({e})', 'f64': f'(long)({e})', 'i64': f'(long)(({e}) % 1000003LL)'}.get(kind, f'(long)({e})')
funcs = []
for i in range(n):
    ret = rng.choice(RET)
    params = [rng.choice(ARG) for _ in range(rng.randint(0, 7))]
    funcs.append((i, ret, params))
callee, caller = [HEAD], [HEAD]
ID = {k: v for k, v in SCALARS.items()}
for k, t in ID.items():
    callee.append(f'{t} id_{k}({t} x) {{ return x; }}')
    caller.append(f'{t} id_{k}({t} x);')
callee.append('short cells[8] = {11, -22, 33, -44, 55, -66, 77, -88};')
callee.append('void esc(void *p) { cells[7] += (short)*(unsigned char *)p; }')
caller.append('void esc(void *p);')
caller.append('''struct S4 mk4(short a, short b) { struct S4 s; s.a = a; s.b = b; return s; }
struct S8 mk8(long a, short b, short c) { struct S8 s; s.a = a; s.b = b; s.c = c; return s; }
struct S12 mk12(long a, long b, long c) { struct S12 s; s.a = a; s.b = b; s.c = c; return s; }''')
host = ['#include <stdio.h>', '#include <stdint.h>', '#define __far', 'void report(long v){printf("%ld\\n",(long)(int32_t)v);}']
def sig(f, name=None):
    i, ret, params = f
    r = 'void' if ret == 'void' else SCALARS[ret]
    ps = ', '.join(f'{SCALARS[p]} p{j}' for j, p in enumerate(params)) or 'void'
    return f'{r} {name or f"f{i}"}({ps})'
protos = '\n'.join(sig(f) + ';' for f in funcs)
callee.append(protos); caller.append(protos)
for f in funcs:
    i, ret, params = f
    body = [f'unsigned long h = {i + 1};'] + [f'h = h * 31 + (unsigned long){fold(p, f"p{j}")};' for j, p in enumerate(params)]
    if ret == 'void':
        body.append('cells[h & 7] = (short)h;')
    elif ret in ('np', 'fp'):
        body.append(f'return ({SCALARS[ret]})&cells[h & 7];')
    elif ret in ('s4', 's8', 's12'):
        s = {'s4': 'struct S4', 's8': 'struct S8', 's12': 'struct S12'}[ret]
        body.append(f'{{ {s} r; ' + {'s4': 'r.a = (short)h; r.b = (short)(h >> 3);', 's8': 'r.a = h; r.b = (short)(h >> 2); r.c = (short)(h >> 4);', 's12': 'r.a = h; r.b = h >> 1; r.c = h >> 2;'}[ret] + ' return r; }')
    elif ret == 'i64':
        body.append('return (long long)h * 1000003LL;')
    elif ret in ('f32', 'f64'):
        body.append(f'return ({SCALARS[ret]})(h & 0xFFF) + 0.5;')
    else:
        body.append(f'return ({SCALARS[ret]})h;')
    for j, p in enumerate(params):
        if p not in ('np', 'fp') and rng.random() < 0.25: body.insert(1, f'esc(&p{j});')
    callee.append(sig(f) + ' {\n    ' + '\n    '.join(body) + '\n}')
# the caller: each function called directly and through a pointer, the result reported
calls = []
index = []
for k, f in enumerate(funcs):
    i, ret, params = f
    wrap = lambda p, e: f'id_{p}({e})' if rng.random() < 0.4 else e
    args = ', '.join(wrap(p, value(p, k + j)) for j, p in enumerate(params))
    for how, fn in (('direct', f'f{i}'), ('pointer', f'(*p{i})')):
        call = f'{fn}({args})'
        index.append((i, how, ret, params))
        if ret == 'void': calls.append(f'{call}; report(cells[{k % 8}]);')
        elif ret in ('np', 'fp'): calls.append(f'report((long)*{call});')
        elif ret == 's4': calls.append(f'{{ struct S4 r = {call}; report(r.a * 3L + r.b); }}')
        elif ret == 's8': calls.append(f'{{ struct S8 r = {call}; report(r.a + r.b * 5L + r.c); }}')
        elif ret == 's12': calls.append(f'{{ struct S12 r = {call}; report(r.a ^ r.b ^ r.c); }}')
        elif ret == 'i64': calls.append(f'{{ long long r = {call}; report((long)(r % 1000003LL)); report((long)(r >> 20)); }}')
        elif ret in ('f32', 'f64'): calls.append(f'report((long)({call}));')
        else: calls.append(f'report((long)({call}));')
caller.append('\n'.join(f'{sig(f, "(*p%d)" % f[0])} = f{f[0]};' for f in funcs).replace(' (*p', ' (*p'))
caller.append('int main(void) {\n    ' + '\n    '.join(calls) + '\n    return 0;\n}')
(out / 'callee.c').write_text('\n'.join(callee) + '\n')
(out / 'caller.c').write_text('\n'.join(caller) + '\n')
import re
host = '#include <stdio.h>\n#define __far\nvoid report(int v) { printf("%d\\n", v); }\n' + (out / 'callee.c').read_text().replace(HEAD, HEAD.replace('extern void report(long value);', ''), 1) + (out / 'caller.c').read_text().replace(HEAD, '', 1)
host = re.sub(r'\blong long\b', 'LL64', host)
host = re.sub(r'\blong\b', 'int', host)
host = host.replace('LL64', 'long long')
(out / 'host.c').write_text(host)

lines = []
for i, how, ret, params in index:
    lines += [f'f{i} {how} {ret} {params}'] * (2 if ret == 'i64' else 1)
(out / 'index.txt').write_text('\n'.join(lines) + '\n')
