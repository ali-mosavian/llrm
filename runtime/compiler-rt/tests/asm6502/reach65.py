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
names=set(blocks); order=list(blocks)
# a block that does not end in rts/rti/jmp falls through into the next label
after={a:b for a,b in zip(order,order[1:])}
def data(n):
    return any(re.search(r'\.(res|byte|word|addr|dword|byt|lobytes|hibytes)\b',l.split(';')[0]) for l in blocks[n])
def bss(n):
    return any(re.search(r'\.res\b',l.split(';')[0]) for l in blocks[n])
def falls(n):
    if data(n): return False
    code=[re.sub(r'^[A-Za-z_][\w$@]*:','',l.split(';')[0]).split() for l in blocks[n]]
    code=[c for c in code if c and not c[0].startswith('.')]
    return not code or code[-1][0].lower() not in ('rts','rti','jmp')
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
    if n in after and falls(n): work.append(after[n])
    for line in blocks[n]:
        text=re.sub(r'^[A-Za-z_][\w$@]*:','',line.split(';')[0]).strip()
        if text.startswith('.'): continue  # directives (.export, .segment) name labels without reaching them
        for w in re.findall(r'[A-Za-z_][\w$@]*',text):
            if w in names and w not in seen: work.append(w)
missing = sorted(n for n in names if n not in lab)
if missing:
    sys.exit("reach65: no address for " + " ".join(missing[:5]) + ": link with cl65 -g, or -Ln lists exported labels only and sizes are wrong")
print(sum(size(n) for n in seen if not bss(n)), " ".join(sorted(seen)))
