#!/usr/bin/env python3
"""reach65.py P.S L.TXT ROOT -> bytes of the code reached from label ROOT, then the labels. P.S is the linked asm (labels at column 0), L.TXT the ld65 -Ln file; a label size is the distance to the next label address. Control: reach65_test.sh."""
import re, sys
src=open(sys.argv[1]).read().splitlines(); lab={}
for l in open(sys.argv[2]):
    m=re.match(r'al ([0-9A-F]+) \.(\S+)',l)
    if m: lab[m.group(2)]=int(m.group(1),16)
blocks={}; cur=None
for line in src:
    m=re.match(r'^([A-Za-z_][\w$@]*):',line)
    if m and not m.group(1).startswith('.L'): cur=m.group(1); blocks[cur]=[]
    if cur: blocks[cur].append(line)
names=set(blocks)
addr=sorted(set(lab[n] for n in names if n in lab))
def size(n):
    a=lab.get(n)
    if a is None: return 0
    nxt=[x for x in addr if x>a]
    return (nxt[0]-a) if nxt else 0
seen=set(); work=[sys.argv[3]]
while work:
    n=work.pop()
    if n in seen or n not in blocks: continue
    seen.add(n)
    for line in blocks[n]:
        for w in re.findall(r'[A-Za-z_][\w$@]*',line.split(';')[0]):
            if w in names and w not in seen: work.append(w)
print(sum(size(n) for n in seen), " ".join(sorted(seen)))
