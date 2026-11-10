#!/usr/bin/env python3
"""Check crates/target/llrm-arm64/src/instructions/opcodes.instr against the pin.

    tools/arm64/check_opcodes.py [PIN_INC]

PIN_INC is the pin's AArch64GenInstrInfo.inc (default
~/scratch/llvm-build-aarch64/lib/Target/AArch64/AArch64GenInstrInfo.inc). Expands the
file's placeholders as `llrm_target::instr::parse_opcodes` does and fails if an LLVM
opcode it names is not in the pin. The Rust tests check the file against the producer.
"""
import itertools, os, re, sys

G={32:'W',64:'X'}; F={16:'H',32:'S',64:'D'}
Z={8:'B',16:'H',32:'W',64:'X'}; ZN={8:'0',16:'1',32:'2',64:'3'}
LET={'g':G,'f':F,'s':F,'z':Z,'zn':ZN}
def alts(t): 
    return max([len(m.split('|')) for m in re.findall(r'\{([^}]*\|[^}]*)\}',t)]+[0])
def spell(t,combo,step):
    def rep(m):
        inside=m.group(1)
        if '|' in inside: return inside.split('|')[step]
        return LET[inside][combo[inside[0]]]
    return re.sub(r'\{([^}]*)\}',rep,t)
def expand(path):
    out=[]
    for ln,raw in enumerate(open(path),1):
        c=raw.split('#')[0].split()
        if not c: continue
        assert len(c)==10,(ln,c)
        name,op,shape,widths,cost,fixed,reads,writes,flags,llvm=c
        axes={}
        if widths!='-':
            for it in widths.split(','): axes.setdefault(it[0],[]).append(int(it[1:]))
        n=max(alts(name),alts(llvm),1)
        for combo in itertools.product(*[[(a,b) for b in bs] for a,bs in axes.items()]):
            cb=dict(combo)
            for st in range(n):
                nm=spell(name,cb,st); lv=spell(llvm,cb,st)
                m=re.search(r'\[([^\]]*)\]',lv)
                al=[lv[:m.start()]+x+lv[m.end():] for x in m.group(1).split('|')] if m else [lv]
                d,s=shape.split('/')
                dn=0 if d=='-' else len(d.split(',')); sn=0 if s=='-' else len(s.split(','))
                out.append((nm,al,dn,sn,shape,ln,op))
    return out


def main():
    path = os.path.join(os.path.dirname(__file__), '../../crates/target/llrm-arm64/src/instructions/opcodes.instr')
    inc = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser('~/scratch/llvm-build-aarch64/lib/Target/AArch64/AArch64GenInstrInfo.inc')
    pin = set(re.findall(r'^\s+([A-Za-z0-9_]+)\s+=\s+\d+,', open(inc).read(), re.M))
    rows = expand(path)
    bad = sorted({(r[0], a) for r in rows for a in r[1] if a not in pin})
    print(f'{len(rows)} forms, {len(bad)} opcodes not in the pin')
    for name, opcode in bad:
        print(f'  {name}: {opcode}')
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
