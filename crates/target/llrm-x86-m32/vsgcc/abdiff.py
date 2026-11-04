"""abdiff.py BASE.jsonl NEW.jsonl: rows (harness results, one JSON per line) worse in clocks, instructions or code bytes, and the better ones."""
import json
import sys

COUNTERS = ('clocks', 'ins', 'code')


def load(path):
    return {(r['prog'], r['variant']): r for r in map(json.loads, open(path)) if r.get('prog')}


def worse(base, new):
    """[(key, counter, before, after)] for each counter that rose; a row with an error in either set has no counters."""
    return [
        (key, k, base[key][k], new[key][k])
        for key in sorted(base.keys() & new.keys())
        if 'error' not in base[key] and 'error' not in new[key]
        for k in COUNTERS
        if new[key][k] > base[key][k]
    ]


if __name__ == '__main__':
    base, new = load(sys.argv[1]), load(sys.argv[2])
    bad = worse(base, new)
    for key, k, a, b in bad:
        print(f"WORSE {key[0]} {key[1]} {k} {a} -> {b}")
    better = worse(new, base)
    print(f"{len(bad)} worse, {len(better)} better counters over {len(base.keys() & new.keys())} rows")
