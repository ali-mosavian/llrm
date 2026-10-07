import json, math, os
from pathlib import Path


def summary(P):
    """Per level and counter, llrm against the best of gcc and clang: the geomean over the programs, and the worst one with its program."""
    lines = []
    for lvl in ('O2', 'Os'):
        L = 'llrm' if lvl == 'O2' else 'llrmOs'
        for k in ('ins', 'clocks', 'code'):
            ratios = {p: d[L][k] / max(1, min(d['gcc' + lvl][k], d['clang' + lvl][k])) for p, d in P.items()}
            g = math.exp(sum(math.log(r) for r in ratios.values()) / len(ratios))
            worst = max(ratios, key=ratios.get)
            lines.append(f"geomean llrm/best {lvl} {k}: {g:.2f}")
            lines.append(f"worst llrm/best {lvl} {k}: {ratios[worst]:.2f} ({worst})")
    return lines


if __name__ == '__main__':
    WORK=Path(os.environ.get('VSGCC_WORK', Path.home()/'scratch/vsgcc-work'))
    rows=[json.loads(l) for l in open(WORK/'results.jsonl') if l.startswith('{')]
    P={}
    for r in rows: P.setdefault(r['prog'],{})[r['variant']]=r
    assert all(r['ok'] for r in rows), [(r['prog'], r['variant']) for r in rows if not r['ok']]
    V=['llrm','gccO2','clangO2','llrmOs','gccOs','clangOs']
    print("| program | compiler | instr | mem ops | 486 clk | nops | code B | llrm/best ins | llrm/best clk |"); print("|---|---|--:|--:|--:|--:|--:|--:|--:|")
    for p,d in P.items():
        for lvl in ('O2','Os'):
            L='llrm' if lvl=='O2' else 'llrmOs'; G,C='gcc'+lvl,'clang'+lvl
            for v in (L,G,C):
                r=d[v]; ri=rc=''
                if v==L:
                    ri=f"{r['ins']/min(d[G]['ins'],d[C]['ins']):.2f}"; rc=f"{r['clocks']/min(d[G]['clocks'],d[C]['clocks']):.2f}"
                print(f"| {p} | {v.replace('O2','').replace('Os',' -Os') if v!='llrmOs' else 'llrm -Os'} {'-O2' if v in ('llrm',G,C) and lvl=='O2' else ''} | {r['ins']} | {r['mem']} | {r['clocks']} | {r['nops']} | {r['code']} | {ri} | {rc} |")
    print(*summary(P), sep='\n')
