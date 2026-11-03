#!/usr/bin/env python3
"""reach32.py ELF ROOT -> bytes of the functions reachable from ROOT by call/jmp in a linked x86-32 ELF (symbol sizes), then their names. Control: reach32_test.sh."""
import re, subprocess, sys
def reach(elf, root):
    nm=subprocess.run(['nm','-S','--defined-only',elf],capture_output=True,text=True).stdout
    size={}; addr={}
    for l in nm.splitlines():
        p=l.split()
        if len(p)==4 and p[2] in 'tTwW': size[p[3]]=int(p[1],16); addr[int(p[0],16)]=p[3]
    dis=subprocess.run(['objdump','-d','--no-show-raw-insn','-M','intel',elf],capture_output=True,text=True).stdout
    calls={}; cur=None
    for l in dis.splitlines():
        m=re.match(r'^[0-9a-f]+ <(.+)>:',l)
        if m: cur=m.group(1); calls[cur]=set(); continue
        m=re.search(r'\b(call|jmp)\s+[0-9a-f]+ <([^>+]+)(?:\+0x[0-9a-f]+)?>',l)
        if m and cur and m.group(2)!=cur: calls[cur].add(m.group(2))
    seen=set(); w=[root]
    while w:
        n=w.pop()
        if n in seen or n not in size: continue
        seen.add(n); w+=list(calls.get(n,()))
    return sum(size[n] for n in seen), sorted(seen)
if __name__=='__main__':
    t,names=reach(sys.argv[1],sys.argv[2]); print(t,' '.join(names))
