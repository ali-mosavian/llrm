#!/usr/bin/env python3
"""Per-address sim65 profile keyed by symbol.

    pcprof.py show DBG PROF [--top N] [--functions]
    pcprof.py diff DBG_A PROF_A DBG_B PROF_B [--top N] [--functions]

PROF is the file `tools/sim65-pcprof.sh`'s sim65 writes at exit (`$SIM65_PCPROF`); DBG is the ld65 debug file of the same
binary (`ca65 -g`, `ld65 --dbgfile`), which carries every label, local ones too. An address belongs to the nearest label
at or below it. `--functions` drops the labels the allocator and the block placer make inside a function (`_b12`, `_pa3`,
`_cz4`, `_cm5`, `_far6`, `__tm37`) so the lines are functions.
"""
import re
import sys

LOCAL = re.compile(r"(_b\d+|_pa\d+|_cz\d+|_cm\d+(t)?|_far\d+|__tm\d+|_ci_.*_\d+)$")
SYM = re.compile(r'^sym\tid=\d+,name="([^"]+)".*?val=0x([0-9A-Fa-f]+).*?type=lab')


def symbols(dbg_lines, functions=False):
    """[(address, name)] sorted by address from an ld65 debug file."""
    found = []
    for line in dbg_lines:
        match = SYM.match(line)
        if match and not (functions and LOCAL.search(match[1])):
            found.append((int(match[2], 16), match[1]))
    return sorted(found)


def profile(prof_lines):
    """{address: (count, cycles)} from the sim65 profile."""
    out = {}
    for line in prof_lines:
        addr, count, cycles = line.split()
        out[int(addr, 16)] = (int(count), int(cycles))
    return out


def by_symbol(syms, prof):
    """{name: (instructions, cycles)}: each address goes to the nearest label at or below it; none: `?`."""
    out = {}
    starts = [a for a, _ in syms]
    import bisect

    for addr, (count, cycles) in prof.items():
        at = bisect.bisect_right(starts, addr) - 1
        name = syms[at][1] if at >= 0 else "?"
        n, c = out.get(name, (0, 0))
        out[name] = (n + count, c + cycles)
    return out


def table(rows, top):
    return "\n".join(f"{c:>12} cy {n:>10} insns  {name}" for name, (n, c) in sorted(rows.items(), key=lambda r: -r[1][1])[:top])


def diff(one, other):
    """{name: (delta instructions, delta cycles)} over the names of either run."""
    names = set(one) | set(other)
    return {name: tuple(b - a for a, b in zip(one.get(name, (0, 0)), other.get(name, (0, 0)))) for name in names}


def main(argv):
    top = int(argv[argv.index("--top") + 1]) if "--top" in argv else 20
    functions = "--functions" in argv
    args = [a for i, a in enumerate(argv) if not a.startswith("--") and not (i and argv[i - 1] == "--top")]
    read = lambda path: open(path).read().splitlines()
    if args[0] == "show":
        print(table(by_symbol(symbols(read(args[1]), functions), profile(read(args[2]))), top))
    elif args[0] == "diff":
        one = by_symbol(symbols(read(args[1]), functions), profile(read(args[2])))
        other = by_symbol(symbols(read(args[3]), functions), profile(read(args[4])))
        rows = diff(one, other)
        for name, (n, c) in sorted(rows.items(), key=lambda r: -abs(r[1][1]))[:top]:
            print(f"{c:>+12} cy {n:>+10} insns  {name}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
