#!/usr/bin/env python3
"""Random one-operation bodies for the InstSimplify differential check: `tools/simplify_cases.py N SEED OUT.ll`, then
`opt -passes=instsimplify -S OUT.ll` against `cargo test -p llrm-mir --lib simplify_cases -- --ignored` (see simplify_tests.rs)."""
import random,sys,subprocess,re,os
random.seed(int(sys.argv[2]) if len(sys.argv)>2 else 1)
N=int(sys.argv[1])
TYPES=['i8','i16','i32']
def leaves(ty):
    c={'i8':[0,1,2,3,7,8,127,128,255,-1],'i16':[0,1,2,15,16,255,32767,-1,-32768,256],'i32':[0,1,2,31,32,255,-1,65536,2147483647]}[ty]
    return c
cases=[]
for k in range(N):
    ty=random.choice(TYPES)
    # predefs: values LLVM cannot simplify
    pre=[('x',ty),('y',ty)]
    defs=[]
    # derived predefined values over x,y (kept alive by stores)
    derived=[('nx',f'xor {ty} %x, -1'),('ng',f'sub {ty} 0, %x'),('ay',f'add {ty} %x, %y'),('sy',f'sub {ty} %y, %x'),('mx',f'mul {ty} %x, %y'),('ox',f'or {ty} %x, %y'),('ax',f'and {ty} %x, %y'),('xx',f'xor {ty} %x, %y'),('sh',f'shl {ty} %x, 3'),('ls',f'lshr {ty} %x, 3')]
    use=random.sample(derived,random.randint(0,4))
    names=['x','y']+[n for n,_ in use]
    def operand():
        r=random.random()
        if r<0.65: return '%'+random.choice(names)
        return str(random.choice(leaves(ty)))
    op=random.choice(['add','sub','mul','and','or','xor','shl','lshr','ashr','udiv','sdiv','urem','srem','icmp','select','zext','trunc','sext','phi'])
    flags=''
    if op in ('add','sub','mul','shl'): flags=random.choice(['','','nuw ','nsw ','nuw nsw '])
    if op in ('udiv','sdiv','lshr','ashr'): flags=random.choice(['','','exact '])
    a,b=operand(),operand()
    rty=ty
    if op=='icmp':
        pred=random.choice(['eq','ne','ult','ule','ugt','uge','slt','sle','sgt','sge'])
        t=f'icmp {pred} {ty} {a}, {b}'; rty='i1'
    elif op=='select':
        c=random.choice(['%c','true','false'])
        t=f'select i1 {c}, {ty} {a}, {ty} {b}'
    elif op in ('zext','sext'):
        wide={'i8':'i16','i16':'i32','i32':'i64'}[ty]
        t=f'{op} {ty} {a if a.startswith("%") else "%x"} to {wide}'; rty=wide
        if random.random()<0.5:
            # compare of extensions
            pass
    elif op=='trunc':
        narrow={'i8':'i1','i16':'i8','i32':'i16'}[ty]
        t=f'trunc {ty} {a if a.startswith("%") else "%x"} to {narrow}'; rty=narrow
    elif op=='phi':
        t=f'phi {ty} [ {a}, %p1 ], [ {b}, %p2 ]'
    else:
        t=f'{op} {flags}{ty} {a}, {b}'
    # second layer: sometimes wrap an extension pair/compare of ext
    pre_text=''.join(f'  %{n} = {e}\n' for n,e in use)
    sink=''.join(f'  store volatile {ty} %{n}, ptr @g_{ty}\n' for n,_ in use)
    if op=='phi':
        body=f'  br i1 %c, label %p1, label %p2\np1:\n  br label %j\np2:\n  br label %j\nj:\n  %t = {t}\n  store volatile {rty} %t, ptr @g_{rty}\n  ret void\n'
        text=f'define void @f{k}({ty} %x, {ty} %y, i1 %c) {{\nb0:\n{pre_text}{sink}'+body+'}\n'
    else:
        text=f'define void @f{k}({ty} %x, {ty} %y, i1 %c) {{\nb0:\n{pre_text}{sink}  %t = {t}\n  store volatile {rty} %t, ptr @g_{rty}\n  ret void\n}}\n'
    cases.append(text)
hdr=''.join(f'@g_{t} = global {t} 0\n' for t in ['i1','i8','i16','i32','i64'])
open(sys.argv[3] if len(sys.argv)>3 else '/tmp/claude-1000/cases.ll','w').write(hdr+'\n'.join(cases))
