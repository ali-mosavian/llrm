#!/usr/bin/env python3
"""profile_mos.py DIR [OPS] [TOP]: per-function cycles of the llvm-mos (compiler-rt) binary64 add/sub/mul/div programs that bench_f64.py
left in DIR as m0_<op>(.elf). mos-sim itself supplies the cycles (--profile: cycles per PC; --trace: every instruction with its opcode),
so no second interpreter is involved; functions come from llvm-nm, calls and inclusive cycles from JSR/RTS in the trace.
Table: top functions by exclusive cycles per record, then the opcode-class histogram of the library code.
Env: LLVM_MOS (the SDK directory), ANNOTATE=name (that function's llvm-objdump listing with the executed count per instruction)."""
import bisect, os, re, subprocess, sys
import profile6502 as P

DIR = sys.argv[1]; OPS = sys.argv[2].split() if len(sys.argv) > 2 else ['add', 'sub', 'mul', 'div']
TOP = int(sys.argv[3]) if len(sys.argv) > 3 else 12
BIN = os.environ['LLVM_MOS'] + '/bin'; N = 100
# the driver and its printf of the totals are not the operation
DRIVER = re.compile(r'^(main|_start|_init|_fini|exit|_Exit|__after_main|__do_init_stack|printf|putchar|vfprintf|_ZN12_GLOBAL.*)$')

def run(op):
    img = f'{DIR}/m0_{op}'
    p = subprocess.run([f'{BIN}/mos-sim', '--cycles', '--trace', '--profile', img], capture_output=True, text=True)
    nm = subprocess.run([f'{BIN}/llvm-nm', '-n', f'{img}.elf'], capture_output=True, text=True).stdout
    syms = [(int(a, 16), n) for a, t, n in (l.split() for l in nm.splitlines() if len(l.split()) == 3) if t in 'tT' and not n.startswith('__fini') and not n.startswith('__init')]
    entry = {}
    for a, n in syms: entry.setdefault(a, n)
    starts = sorted(entry)
    def func(pc):
        i = bisect.bisect_right(starts, pc) - 1
        return starts[i] if i >= 0 else None
    cyc = {}; total = None; ops = {}; calls = {}; incl = {}; stack = []; active = {}; execs = {}
    for l in p.stderr.splitlines():
        m = re.match(r'^([0-9a-f]{4}) a:', l)
        if m:
            pc = int(m.group(1), 16); opc = int(re.search(r'insn:([0-9a-f]{2})', l).group(1), 16)
            execs[pc] = execs.get(pc, 0) + 1; ops[pc] = opc
            continue
        m = re.match(r'^([0-9a-f]{4}) (\d+)$', l)
        if m: cyc[int(m.group(1), 16)] = int(m.group(2))
        m = re.match(r'^(\d+) cycles', l)
        if m: total = int(m.group(1))
    # JSR pushes its target, RTS pops it
    lines = [l for l in p.stderr.splitlines() if re.match(r'^[0-9a-f]{4} a:', l)]
    # cycles of each executed instruction are its PC's profile cycles over its count: exact when the instruction always costs the same,
    # so inclusive cycles use the profile average per PC
    avg = {pc: cyc.get(pc, 0) / n for pc, n in execs.items()}
    for l in lines:
        pc = int(l[:4], 16); opc = int(l[l.index('insn:') + 5:l.index('insn:') + 7], 16)
        for fr in stack: fr[1] += avg[pc]
        if opc == 0x20:
            tgt = int(l[l.index('insn:') + 11:l.index('insn:') + 13] + l[l.index('insn:') + 8:l.index('insn:') + 10], 16)
            calls[tgt] = calls.get(tgt, 0) + 1
            stack.append([tgt, 0.0]); active[tgt] = active.get(tgt, 0) + 1
        elif opc == 0x60 and stack:
            f, c = stack.pop(); active[f] -= 1
            if not active[f]: incl[f] = incl.get(f, 0) + c
    ex = {}
    for pc, c in cyc.items():
        f = func(pc)
        if f is not None: ex[f] = ex.get(f, 0) + c
    return total, entry, ex, calls, incl, cyc, execs, ops, func

def main():
    for op in OPS:
        total, entry, ex, calls, incl, cyc, execs, ops, func = run(op)
        lib = {f: c for f, c in ex.items() if not DRIVER.match(entry[f])}
        libtot = sum(lib.values())
        print(f'\n== {op} (llvm-mos -Os, mos-sim): run total {total} cycles; library code {libtot / N:.0f} cycles/record')
        print('%-26s %8s %10s %10s %6s' % ('function (per record)', 'calls', 'excl', 'incl', 'excl%'))
        for f, c in sorted(lib.items(), key=lambda kv: -kv[1])[:TOP]:
            print('%-26s %8.1f %10.0f %10.0f %5.1f%%' % (entry[f][:26], calls.get(f, 0) / N, c / N, incl.get(f, 0) / N, 100 * c / libtot))
        hist = {c: [0, 0] for c in P.CLASSES}
        for pc, n in execs.items():
            f = func(pc)
            if f is None or DRIVER.match(entry[f]): continue
            cl = P._T[ops[pc]][3]; hist[cl][0] += n; hist[cl][1] += cyc.get(pc, 0)
        print('class (library code)  insns/rec cycles/rec   cyc%')
        for c in P.CLASSES: print('%-8s %20.0f %9.0f %6.1f%%' % (c, hist[c][0] / N, hist[c][1] / N, 100 * hist[c][1] / libtot))
        if os.environ.get('ANNOTATE'): annotate(op, execs, entry)

def annotate(op, execs, entry):
    name = os.environ['ANNOTATE']
    out = subprocess.run([f'{BIN}/llvm-objdump', '-d', '--no-show-raw-insn', f'--disassemble-symbols={name}', f'{DIR}/m0_{op}.elf'], capture_output=True, text=True).stdout
    for l in out.splitlines():
        m = re.match(r'^\s+([0-9a-f]+):\s+(.*)$', l)
        if m: print('  %04X %9s  %s' % (int(m.group(1), 16), execs.get(int(m.group(1), 16), ''), m.group(2)))
        else: print(l)

main()
