"""abdiff.py BASE.jsonl NEW.jsonl --repo R --base SHA --head SHA --base-bin P --head-bin P: rows (harness results, one JSON per line) worse in clocks, instructions or code bytes, and the better ones. It refuses unless the head sits on the base, the binaries differ and both sets have rows."""
import argparse
import hashlib
import json
import subprocess
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


def refusals(repo, base_sha, head_sha, base_bin, head_bin, base, new):
    """Why this A/B proves nothing: the merge-base of head and base is not base, the binaries are one, or a set is empty."""
    out = []
    mb = subprocess.run(['git', '-C', repo, 'merge-base', head_sha, base_sha], capture_output=True, text=True).stdout.strip()
    if mb != subprocess.run(['git', '-C', repo, 'rev-parse', base_sha], capture_output=True, text=True).stdout.strip():
        out.append(f"head {head_sha} does not sit on base {base_sha} (merge-base {mb or 'none'})")
    md5 = lambda p: hashlib.md5(open(p, 'rb').read()).hexdigest()
    if md5(base_bin) == md5(head_bin):
        out.append('base and head binaries are md5-identical')
    if not base or not new:
        out.append(f"empty result set (base {len(base)} rows, new {len(new)} rows)")
    return out


if __name__ == '__main__':
    ap = argparse.ArgumentParser()
    ap.add_argument('base_rows')
    ap.add_argument('new_rows')
    for flag in ('repo', 'base', 'head', 'base-bin', 'head-bin'):
        ap.add_argument('--' + flag, required=True)
    a = ap.parse_args()
    base, new = load(a.base_rows), load(a.new_rows)
    why = refusals(a.repo, a.base, a.head, a.base_bin, a.head_bin, base, new)
    if why:
        sys.exit('REFUSED: ' + '; '.join(why))
    bad = worse(base, new)
    for key, k, a, b in bad:
        print(f"WORSE {key[0]} {key[1]} {k} {a} -> {b}")
    better = worse(new, base)
    print(f"{len(bad)} worse, {len(better)} better counters over {len(base.keys() & new.keys())} rows")
