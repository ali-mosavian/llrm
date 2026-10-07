import json, math, sys, re, collections
G = '/home/alim/scratch/llvm-spike/gate/'
tag = sys.argv[1] if len(sys.argv) > 1 else 'llvm'
base = json.load(open(G + 'base.json'))
M = ('instructions', 'clocks', 'code_bytes')
def cause(e):
    for pat, name in [(r'far constant', 'far constant (addrspace)'), (r'memmove whose direction', 'memmove direction fact'),
                      (r'an address of no global', 'constant address / addrspacecast expr'), (r'usub\.sat|uadd\.sat|intrinsic MIR does not have', 'intrinsic MIR lacks'),
                      (r'llvm\.(smin|smax)', 'smin/smax on long (m16 backend)'), (r'prints', 'wrong result'), (r'expected a constant', 'metadata operand'),
                      (r'no main body', 'entry body lost'), (r'timed out|Timeout', 'backend does not terminate (>300 s)'), (r'expected', 'parse')]:
        if re.search(pat, e): return name
    return e[:80].replace('\n', ' ')
for lvl in ('O2', 'Os'):
    new = json.load(open(f'{G}m16-{tag}-{lvl}.json'))
    print(f'## m16 {lvl}')
    for lang in ('c', 'nib', 'bas'):
        rat = {k: {} for k in M}; causes = collections.Counter(); n = 0; folded = []
        for key, b in base.items():
            name, l, o = key.rsplit('/', 2)
            if l != lang or o != lvl: continue
            n += 1
            r = new.get(key)
            if r is None or 'error' in r or 'error' in b or 'known' in b or 'known' in r or 'instructions' not in r:
                causes[cause((r or {}).get('error', 'known' if r is not None and 'known' in r else 'missing'))] += 1; continue
            if r['instructions'] / max(1, b['instructions']) < 0.25: causes['folded to <25% (excluded, listed)'] += 1; folded.append(name); continue
            for k in M: rat[k][name] = r[k] / max(1, b[k])
        out = [f'{lang:3} rows {len(rat["instructions"])}/{n}']
        for k in M:
            if not rat[k]: continue
            g = math.exp(sum(map(math.log, rat[k].values())) / len(rat[k])); w = max(rat[k], key=rat[k].get)
            out.append(f'{k[:5]} geo {g:.3f} worst {rat[k][w]:.2f} ({w})')
        print('  '.join(out)); print('     not counted:', dict(causes), folded or '')
