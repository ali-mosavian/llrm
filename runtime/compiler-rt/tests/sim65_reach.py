#!/usr/bin/env python3
"""sim65_reach.py OUTDIR ROOT FILE.s...: copy each llrm-cc 6502 .s to OUTDIR, keeping only the functions reachable from ROOT.

ld65 links whole objects, so int64.s would bring its 64-bit divide along for a shift. llrm-cc starts every function with a
`.segment "BSS"` line (its static cells), then CODE and the body: that line splits the file. A name resolves in its own file first,
then to an exported name in another file. Roots may be given as ROOT,ROOT2,... Prints the functions kept per file."""
import os, re, sys

LABEL = re.compile(r'^([A-Za-z_][A-Za-z_0-9]*):')
TOKEN = re.compile(r'[A-Za-z_][A-Za-z_0-9]*')
EXPORT = re.compile(r'^\.export (\S+)')


def load(path):
    lines = open(path).read().split('\n')
    cuts = [i for i, l in enumerate(lines) if l == '.segment "BSS"']
    first = next(i for i, l in enumerate(lines) if l and not re.match(r'^\w+ = ', l))
    bounds = sorted(set([first] + cuts)) + [len(lines)]
    head = lines[:first]
    chunks = [lines[a:b] for a, b in zip(bounds, bounds[1:])]
    defs, exports = {}, {}
    for k, c in enumerate(chunks):
        for l in c:
            m = LABEL.match(l)
            if m: defs[m.group(1)] = k
            m = EXPORT.match(l)
            if m: exports[m.group(1)] = k
    return head, chunks, defs, exports


def main():
    out, roots, paths = sys.argv[1], sys.argv[2].split(','), sys.argv[3:]
    files = {p: load(p) for p in paths}
    todo = []
    keep = {p: set() for p in paths}

    def want(p, name):
        d = files[p][2]
        if name in d:
            todo.append((p, d[name])); return
        for q in paths:
            if name in files[q][3]:
                todo.append((q, files[q][3][name])); return

    for r in roots: want(paths[0], r)
    while todo:
        p, k = todo.pop()
        if k in keep[p]: continue
        keep[p].add(k)
        for l in files[p][1][k]:
            if not l.startswith(' ') or l.lstrip().startswith('.'): continue
            for t in TOKEN.findall(l.split(';')[0])[1:]:
                want(p, t)
    os.makedirs(out, exist_ok=True)
    for p in paths:
        head, chunks, defs, _ = files[p]
        body = [l for k, c in enumerate(chunks) if k in keep[p] for l in c]
        open(os.path.join(out, os.path.basename(p)), 'w').write('\n'.join(head + body) + '\n')
        print(os.path.basename(p), len(keep[p]), 'of', len(chunks), 'functions kept')


main()
