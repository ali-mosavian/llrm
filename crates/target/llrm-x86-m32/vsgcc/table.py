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


def summary(P, label=''):
    """Per level and counter, llrm against gcc and against the best of gcc and clang at that level: the geomean over the programs,
    and the worst one with its program."""
    lines = []
    for lvl in LEVELS:
        for k in ('ins', 'clocks', 'code'):
            for name, ref in (('best', lambda d: min(d['gcc' + lvl][k], d['clang' + lvl][k])), ('gcc', lambda d: d['gcc' + lvl][k])):
                ratios = {p: d[llrm(lvl)][k] / max(1, ref(d)) for p, d in P.items()}
                g = math.exp(sum(math.log(r) for r in ratios.values()) / len(ratios))
                worst = max(ratios, key=ratios.get)
                lines.append(f"{label}geomean llrm/{name} {lvl} {k}: {g:.2f}")
                lines.append(f"{label}worst llrm/{name} {lvl} {k}: {ratios[worst]:.2f} ({worst})")
    return lines


def grouped(P):
    """The bench programs and the `x_` kernels (kernels/) as two summaries: the kernels were written to find what the bench programs do not."""
    lines = []
    for label, keep in (('bench ', lambda p: not p.startswith('x_')), ('x_ kernels ', lambda p: p.startswith('x_'))):
        group = {p: d for p, d in P.items() if keep(p)}
        if group:
            lines += summary(group, f"{label}(n={len(group)}) ")
    return lines


if __name__ == '__main__':
    WORK=Path(os.environ.get('VSGCC_WORK', Path.home()/'scratch/vsgcc-work'))
    rows=[json.loads(l) for l in open(WORK/'results.jsonl') if l.startswith('{')]
    P={}
    for r in rows: P.setdefault(r['prog'],{})[r['variant']]=r
    complete(P)
    assert all(r['ok'] for r in rows), [(r['prog'], r['variant']) for r in rows if not r['ok']]
    print("| program | compiler | instr | mem ops | 486 clk | nops | code B | llrm/best ins | llrm/best clk |"); print("|---|---|--:|--:|--:|--:|--:|--:|--:|")
    for p,d in P.items():
        for lvl in LEVELS:
            L, G, C = llrm(lvl), 'gcc'+lvl, 'clang'+lvl
            for v in (L,G,C):
                r=d[v]; ri=rc=''
                if v==L:
                    ri=f"{r['ins']/min(d[G]['ins'],d[C]['ins']):.2f}"; rc=f"{r['clocks']/min(d[G]['clocks'],d[C]['clocks']):.2f}"
                print(f"| {p} | {v[:-2] if v[-2:] == lvl else v} -{lvl} | {r['ins']} | {r['mem']} | {r['clocks']} | {r['nops']} | {r['code']} | {ri} | {rc} |")
    print(*grouped(P), sep='\n')
