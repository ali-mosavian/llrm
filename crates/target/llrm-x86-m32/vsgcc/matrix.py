"""matrix.py RESULTS.jsonl...: scoreboard TSV rows (target prog level compiler code_bytes clocks instrs note), later files overriding earlier. code_bytes = .text plus .data/.rodata, as table.py's gate."""
import json
import sys

LEVELS = {'llrm': ('O2', 'ours'), 'llrmO1': ('O1', 'ours'), 'llrmO3': ('O3', 'ours'), 'llrmOs': ('Os', 'ours')}
ORDER = {'O1': 0, 'O2': 1, 'O3': 2, 'Os': 3}


def split(variant):
    if variant in LEVELS:
        return LEVELS[variant]
    for c in ('gcc', 'clang'):
        if variant.startswith(c):
            return variant[len(c):], c


def rows(results):
    out = {}
    for x in results:
        level, compiler = split(x['variant'])
        out[(x['prog'], level, compiler)] = ['x86-m32', x['prog'], level, compiler, str(x['code'] + x.get('data', 0)), str(x['clocks']), str(x['ins']), '']
    return [out[k] for k in sorted(out, key=lambda k: (k[0], ORDER[k[1]], k[2]))]


if __name__ == '__main__':
    results = [json.loads(l) for path in sys.argv[1:] for l in open(path) if l.startswith('{')]
    for row in rows(results):
        print('\t'.join(row))
