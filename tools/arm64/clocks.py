"""Modelled clocks of an arm64 ELF: executed addresses from unicorn x `llvm-mca -mcpu=apple-m1` per basic block.

A MODEL, not a measurement: each basic block is priced alone (no cache, no branch misprediction, no overlap across blocks),
so every figure it prints says "modelled". A block that is its own loop is priced in steady state (cycles per iteration),
any other block once, pipeline fill included.

    clocks.py ELF --kernel|--whole    modelled clocks and executed instructions of the compiled kernels, or of the program
                                      with its runtime (the range is chosen, never defaulted: count.py's rule)
"""
import argparse, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
LLVM = os.environ.get('LLVM_BIN', os.path.expanduser('~/scratch/llvm-build-aarch64/bin'))
ENDS_BLOCK = re.compile(r'^(b|bl|br|blr|ret|cbz|cbnz|tbz|tbnz|b\.\w+)\s')
CALL = re.compile(r'^(bl|blr)\s')
STEADY_ITERATIONS = 100


def disassemble(elf):
    """{address: text}: the instructions of `elf`, branch targets rewritten to a fixed offset llvm-mca can parse."""
    out = subprocess.run([f'{LLVM}/llvm-objdump', '-d', '--no-show-raw-insn', elf], capture_output=True, text=True, check=True).stdout
    code = {}
    for line in out.splitlines():
        m = re.match(r'\s*([0-9a-f]+):\s+(\S.*)', line)
        if m:
            code[int(m.group(1), 16)] = m.group(2).split('//')[0].strip()
    return code


def target_of(text):
    m = re.search(r'\b0x([0-9a-f]+) <', text)
    return int(m.group(1), 16) if m else None


def blocks(code, executed):
    """The basic blocks the program executed, as lists of consecutive addresses: cut after a branch and before a branch target."""
    leaders = set()
    for address, text in code.items():
        if ENDS_BLOCK.match(text + ' '):
            leaders.add(address + 4)
            if target_of(text) is not None:
                leaders.add(target_of(text))
    found, current = [], []
    for address in sorted(a for a in executed if a in code):
        if current and (address != current[-1] + 4 or address in leaders):
            found.append(current)
            current = []
        current.append(address)
    if current:
        found.append(current)
    return found


def mca_cycles(lines, iterations):
    """Cycles per iteration of `lines` repeated `iterations` times, from llvm-mca, or None if it did not run."""
    # llvm-mca assumes 100 cycles for any call; a call here is the branch that ends the block, one cycle.
    calls = sum(1 for line in lines if CALL.match(line + ' '))
    lines = [line for line in lines if not CALL.match(line + ' ')]
    if not lines:
        return float(calls)
    text = '\n'.join(re.sub(r'\b0x[0-9a-f]+ <[^>]*>', '#4096' if line.startswith('adrp') else '#16', line) for line in lines)
    run = subprocess.run([f'{LLVM}/llvm-mca', '-mtriple=aarch64-linux-gnu', '-mcpu=apple-m1', f'-iterations={iterations}', '-'], input=text, capture_output=True, text=True)
    cycles, done = re.search(r'Total Cycles:\s+(\d+)', run.stdout), re.search(r'Iterations:\s+(\d+)', run.stdout)
    return int(cycles.group(1)) / int(done.group(1)) + calls if cycles and done else None


def modelled_clocks(code, histogram):
    """(modelled clocks, executed instructions); a block llvm-mca cannot price is an error, not a zero."""
    total, executed = 0.0, 0
    for block in blocks(code, histogram):
        count, text = histogram[block[0]], [code[a] for a in block]
        loops = target_of(text[-1]) == block[0] and ENDS_BLOCK.match(text[-1] + ' ')
        cycles = mca_cycles(text, STEADY_ITERATIONS if loops else 1)
        if cycles is None:
            raise SystemExit(f'clocks.py: llvm-mca could not price the block at {block[0]:#x}')
        total += count * cycles
        executed += count * len(block)
    return total, executed


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
    histogram = {}
    reported, instrs, err = count.run(args.elf, kernel=args.kernel, histogram=histogram)
    if err:
        raise SystemExit(f'clocks.py: {err}')
    clocks, executed = modelled_clocks(disassemble(args.elf), histogram)
    print(f'{os.path.basename(args.elf)}: {clocks:.0f} modelled clocks, {executed} instructions ({"kernel" if args.kernel else "whole"}, llvm-mca apple-m1 per block)')


if __name__ == '__main__':
    main()
