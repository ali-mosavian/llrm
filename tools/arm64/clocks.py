"""Modelled clocks of an arm64 ELF: `llvm-mca -mcpu=apple-m1` over the trace of addresses unicorn executed.

A MODEL, not a measurement (no cache, no branch misprediction), so every figure it prints says "modelled". Instructions are
priced in the order they ran, so they overlap across branches and loop trips; a long trace is cut into windows.

    clocks.py ELF --kernel|--whole    modelled clocks and executed instructions of the compiled kernels, or of the program
                                      with its runtime (the range is chosen, never defaulted: count.py's rule)
"""
import argparse, array, hashlib, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
LLVM = os.environ.get('LLVM_BIN', '/usr/lib/llvm-20/bin')
CALL = re.compile(r'^(bl|blr)\s')
WINDOW = 100_000  # instructions llvm-mca takes at once, at about 1.5 KB of its memory each
# A store that updates its base register: `str w1, [x0], #2` (post) or `str w1, [x0, #2]!` (pre).
POST_STORE = re.compile(r'^(st(?:r|rb|rh|p))\s+(.+?),\s*\[(x\d+|sp)\],\s*(#-?(?:0x)?[0-9a-f]+)$')
PRE_STORE = re.compile(r'^(st(?:r|rb|rh|p))\s+(.+?),\s*\[(x\d+|sp),\s*(#-?(?:0x)?[0-9a-f]+)\]!$')


def disassemble(elf):
    """{address: text}: the instructions of `elf`, branch targets rewritten to a fixed offset llvm-mca can parse."""
    out = subprocess.run([f'{LLVM}/llvm-objdump', '-d', '--no-show-raw-insn', elf], capture_output=True, text=True, check=True).stdout
    code = {}
    for line in out.splitlines():
        m = re.match(r'\s*([0-9a-f]+):\s+(\S.*)', line)
        if m:
            code[int(m.group(1), 16)] = m.group(2).split('//')[0].strip()
    return code


def split_writeback(line):
    """The lines for `line`: a store that updates its base becomes the plain store and an `add`.

    llvm-mca makes the updated base wait for the store's data, so a loop that loads through x8 and stores `[x8], #2`
    carries load latency round the loop; a core updates the address apart from the data."""
    post, pre = POST_STORE.match(line), PRE_STORE.match(line)
    if not post and not pre:
        return [line]
    op, data, base, step = (post or pre).groups()
    bump = f'sub {base}, {base}, #{step[2:]}' if step.startswith('#-') else f'add {base}, {base}, {step}'
    return [f'{op} {data}, [{base}]', bump] if post else [bump, f'{op} {data}, [{base}]']


def mca_cycles(lines, iterations):
    """Cycles per iteration of `lines` repeated `iterations` times, from llvm-mca, or None if it did not run."""
    # llvm-mca assumes 100 cycles for any call; a call here is the branch that ends the block, one cycle.
    calls = sum(1 for line in lines if CALL.match(line + ' '))
    lines = [part for line in lines if not CALL.match(line + ' ') for part in split_writeback(line)]
    if not lines:
        return float(calls)
    # objdump prints a target no symbol covers bare (`adr x0, 0x200158`, as gcc takes a nearby literal): llvm-mca takes an
    # offset. An immediate (`#0x1`, `#-0x1`) is not a target.
    target = re.compile(r'(?<![#\w-])0x[0-9a-f]+( <[^>]*>)?$')
    text = '\n'.join(target.sub('#4096' if line.startswith('adrp') else '#16', line) for line in lines)
    run = subprocess.run([f'{LLVM}/llvm-mca', '-mtriple=aarch64-linux-gnu', '-mcpu=apple-m1', f'-iterations={iterations}', '-'], input=text, capture_output=True, text=True)
    cycles, done = re.search(r'Total Cycles:\s+(\d+)', run.stdout), re.search(r'Iterations:\s+(\d+)', run.stdout)
    return int(cycles.group(1)) / int(done.group(1)) + calls if cycles and done else None


def modelled_clocks(code, trace):
    """(modelled clocks, executed instructions) of `trace`; a window llvm-mca cannot price is an error, not a zero."""
    total, priced = 0.0, {}
    for start in range(0, len(trace), WINDOW):
        window = trace[start:start + WINDOW]
        key = hashlib.sha256(array.array('L', window).tobytes()).digest()
        if key not in priced:
            priced[key] = mca_cycles([code[a] for a in window], 1)
        if priced[key] is None:
            raise SystemExit(f'clocks.py: llvm-mca could not price the window from {window[0]:#x}')
        total += priced[key]
    return total, len(trace)


def parser():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('elf')
    where = ap.add_mutually_exclusive_group(required=True)
    where.add_argument('--kernel', action='store_true', help='price the compiled kernels only')
    where.add_argument('--whole', action='store_true', help='price the whole program, runtime included')
    return ap


def main():
    args = parser().parse_args()
    sys.path.insert(0, HERE)
    import count
    trace = array.array('L')
    reported, instrs, err = count.run(args.elf, kernel=args.kernel, trace=trace)
    if err:
        raise SystemExit(f'clocks.py: {err}')
    clocks, executed = modelled_clocks(disassemble(args.elf), trace)
    print(f'{os.path.basename(args.elf)}: {clocks:.0f} modelled clocks, {executed} instructions ({"kernel" if args.kernel else "whole"}, llvm-mca apple-m1 over the trace)')


if __name__ == '__main__':
    main()
