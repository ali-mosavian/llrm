import json, math, os
from pathlib import Path


LEVELS = ('O1', 'O2', 'O3', 'Os')


def llrm(level):
    """`llrm` is -O2: the name the results had before the other levels."""
    return 'llrm' if level == 'O2' else 'llrm' + level


VARIANTS = [name for level in LEVELS for name in (llrm(level), 'gcc' + level, 'clang' + level)]


def complete(P):
    """Every program has a row for every variant: a link that failed leaves none, and a table of what is left reads as a result."""
    missing = [(p, v) for p, d in P.items() for v in VARIANTS if v not in d]
    assert not missing, f"no result for {missing}"


def ratios(P, lvl, k, reference):
    """llrm over `reference` ('gcc' or 'clang') at one level and counter, per program."""
    return {p: d[llrm(lvl)][k] / max(1, d[reference + lvl][k]) for p, d in P.items()}


def summary(P, label=''):
    """Per level, llrm against gcc: the geomean over the programs beside the worst one, clocks first, then instructions and code.
    The ratio against the best of gcc and clang took each counter's minimum from whichever compiler had it (x_iir: clang's three
    imuls for instructions, gcc's chains for clocks, where clang runs twice gcc's clocks), so no compiler had that ratio.
    clang follows for reference."""
    lines = []
    for reference in ('gcc', 'clang'):
        for lvl in LEVELS:
            for k in ('clocks', 'ins', 'code'):
                by = ratios(P, lvl, k, reference)
                g = math.exp(sum(math.log(r) for r in by.values()) / len(by))
                worst = max(by, key=by.get)
                lines.append(f"{label}geomean llrm/{reference} {lvl} {k}: {g:.2f}")
                lines.append(f"{label}worst llrm/{reference} {lvl} {k}: {by[worst]:.2f} ({worst})")
    return lines


def grouped(P):
    """The bench programs and the `x_` kernels (kernels/) as two summaries: the kernels were written to find what the bench programs do not."""
    lines = []
    for label, keep in (('bench ', lambda p: not p.startswith('x_')), ('x_ kernels ', lambda p: p.startswith('x_'))):
        group = {p: d for p, d in P.items() if keep(p)}
        if group:
            lines += summary(group, f"{label}(n={len(group)}) ")
    return lines


def width(rows):
    """The one long double width, in bits, of every row: a table of 64 beside 80 puts Watcom's long double beside gcc's."""
    widths = {r.get('ld', 64) for r in rows}
    if len(widths) != 1:
        raise ValueError(f"rows of long double widths {sorted(widths)} in one table")
    return widths.pop()


def compilers(rows):
    """The one compiler stamp (build.sh: md5 of llrm-cc + llrm-llc) of every row: a file of two, or of an unstamped row, says nothing of what was measured."""
    if any('cc' not in r for r in rows):
        raise ValueError("rows with no compiler stamp: rebuild with build.sh")
    stamps = {r['cc'] for r in rows}
    if len(stamps) != 1:
        raise ValueError(f"rows of compilers {sorted(stamps)} in one table")
    return stamps.pop()


if __name__ == '__main__':
    WORK=Path(os.environ.get('VSGCC_WORK', Path.home()/'scratch/vsgcc-work'))
    rows=[json.loads(l) for l in open(WORK/'results.jsonl') if l.startswith('{')]
    for r in rows: r['code'] += r.get('data', 0)   # bytes are .text plus data, the order's measure
    P={}
    for r in rows: P.setdefault(r['prog'],{})[r['variant']]=r
    complete(P)
    print(f"long double: {width(rows)} bit on both sides; llrm compiler {compilers(rows)}")
    assert all(r['ok'] for r in rows), [(r['prog'], r['variant']) for r in rows if not r['ok']]
    print("| program | compiler | instr | mem ops | misaligned | 486 clk | nops | text+data B | llrm/gcc clk | llrm/gcc ins | llrm/clang clk |"); print("|---|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|")
    for p,d in P.items():
        for lvl in LEVELS:
            L, G, C = llrm(lvl), 'gcc'+lvl, 'clang'+lvl
            for v in (L,G,C):
                r = d[v]
                cells = ['', '', '']
                if v == L:
                    cells = [f"{r['clocks'] / max(1, d[G]['clocks']):.2f}", f"{r['ins'] / max(1, d[G]['ins']):.2f}", f"{r['clocks'] / max(1, d[C]['clocks']):.2f}"]
                print(f"| {p} | {v[:-2] if v[-2:] == lvl else v} -{lvl} | {r['ins']} | {r['mem']} | {r.get('misaligned', 0)} | {r['clocks']} | {r['nops']} | {r['code']} | " + " | ".join(cells) + " |")
    print(*grouped(P), sep='\n')
