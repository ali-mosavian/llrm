#!/usr/bin/env python3
"""Random one-operation bodies for the InstSimplify differential check: `tools/simplify_cases.py N SEED OUT.ll`, then
`opt -passes=instsimplify -S OUT.ll` against `cargo test -p llrm-mir --lib simplify_cases -- --ignored` (see simplify_tests.rs)."""
import random,sys,subprocess,re,os
random.seed(int(sys.argv[2]) if len(sys.argv)>2 else 1)
N=int(sys.argv[1])
TYPES=['i8','i16','i32']
def leaves(ty):
    c={'i8':[0,1,2,3,7,8,127,128,255,-1],'i16':[0,1,2,15,16,255,32767,-1,-32768,256],'i32':[0,1,2,31,32,255,-1,1 << 16,2147483647]}[ty]
    return c
cases=[]
for k in range(N):
    ty=random.choice(TYPES)
    # predefs: values LLVM cannot simplify
    pre=[('x',ty),('y',ty)]
    defs=[]
    # derived predefined values over x,y (kept alive by stores)
    derived=[('nx',f'xor {ty} %x, -1'),('ng',f'sub {ty} 0, %x'),('ay',f'add {ty} %x, %y'),('sy',f'sub {ty} %y, %x'),('mx',f'mul {ty} %x, %y'),('ox',f'or {ty} %x, %y'),('an',f'and {ty} %x, %y'),('xx',f'xor {ty} %x, %y'),('sh',f'shl {ty} %x, 3'),('ls',f'lshr {ty} %x, 3')]
    derived+=[('au',f'add nuw {ty} %x, %y'),('as',f'add nsw {ty} %x, %y'),('a1',f'add nuw {ty} %x, 1'),('an1',f'add nsw {ty} %x, 1'),('a2',f'add nsw {ty} %x, 2'),('am',f'add {ty} %x, -1'),
              ('ur',f'urem {ty} %x, %y'),('lx',f'lshr {ty} %x, 2'),('ud',f'udiv {ty} %x, 3'),('ue',f'udiv exact {ty} %x, %y'),('le',f'lshr exact {ty} %x, %y'),('sc',f'sub {ty} 5, %x'),('s6',f'sub {ty} 6, %x'),
              ('s4',f'shl {ty} 4, %x'),('sq',f'shl nuw {ty} 4, %x'),('mu',f'mul {ty} %x, 3'),('um',f'udiv {ty} %mu0, 5'),('sn',f'shl nuw {ty} %x, %y'),('sn2',f'shl nuw {ty} %y, %y'),('o8',f'or {ty} %x, 8'),('o9',f'or {ty} %x, 9'),
              ('a3',f'and {ty} %x, 3'),('a7',f'and {ty} %x, 7'),('us',f'call {ty} @llvm.uadd.sat.{ty}({ty} %x, {ty} %y)'),('ub',f'call {ty} @llvm.usub.sat.{ty}({ty} %x, {ty} %y)'),('sd',f'sdiv exact {ty} %x, %y'),('ae',f'ashr exact {ty} %x, %y'),('zn',f'sub {ty} 0, %zz')]
    derived=[d for d in derived if '%mu0' not in d[1] and '%zz' not in d[1]]
    use=random.sample(derived,random.randint(0,5))
    # min/max idioms: a compare and the select on it (both kept alive)
    mm=[('smax','sgt','x','y'),('smin','slt','x','y'),('umax','ugt','x','y'),('umin','ult','x','y'),('smaxy','sgt','y','x'),('uminy','ule','y','x'),('smaxe','sge','x','y'),('umaxe','uge','x','y')]
    mm=random.sample(mm,random.randint(0,3))
    mmdefs=''.join(f'  %c_{n} = icmp {pr} {ty} %{a}, %{b}\n  %{n} = select i1 %c_{n}, {ty} %{a}, {ty} %{b}\n' for n,pr,a,b in mm)
    ik=random.sample([(n,a,b) for n in ('smax','smin','umax','umin') for a,b in (('x','y'),('y','x'))],random.randint(0,3))
    mmdefs+=''.join(f'  %i_{n}{a} = call {ty} @llvm.{n}.{ty}({ty} %{a}, {ty} %{b})\n' for n,a,b in ik)
    mmsink_i=''.join(f'  store volatile {ty} %i_{n}{a}, ptr @g_{ty}\n' for n,a,b in ik)
    mmsink=''.join(f'  store volatile {ty} %{n}, ptr @g_{ty}\n  store volatile i1 %c_{n}, ptr @g_i1\n' for n,_,_,_ in mm)+mmsink_i
    names=['x','y']+[n for n,_ in use]+[n for n,_,_,_ in mm]+[f'i_{n}{a}' for n,a,b in ik]
    def operand():
        r=random.random()
        if r<0.65: return '%'+random.choice(names)
        return str(random.choice(leaves(ty)))
    op=random.choice(['cmpmix','cmpmix','cmpmix','icmp','icmp','icmp','add','sub','mul','and','or','xor','shl','lshr','ashr','udiv','sdiv','urem','srem','icmp','select','zext','trunc','sext','phi','icall','icall','icall','icall','selcmp','selcmp','selcmp','selcmp'])
    flags=''
    if op in ('add','sub','mul','shl'): flags=random.choice(['','','nuw ','nsw ','nuw nsw '])
    if op in ('udiv','sdiv','lshr','ashr'): flags=random.choice(['','','exact '])
    a,b=operand(),operand()
    rty=ty
    extra=''
    selpre=''
    if op=='cmpmix':
        vals=['%'+n for n in names]+['0','0','1','-1']+[str(random.choice(leaves(ty))) for _ in range(3)]
        sub=f'  %dsub = sub {ty} %{random.choice(names)}, %{random.choice(names)}\n'
        adds=f'  %dadd = add {"nuw " if random.random()<0.5 else ""}{ty} %x, {random.choice([1,2,3])}\n'
        extra=sub+adds
        vals+=['%dsub','%dadd']
        def cmp(k):
            pr=random.choice(['eq','ne','ult','ule','ugt','uge','slt','sle','sgt','sge'])
            a,b=random.choice(vals),random.choice(vals)
            if random.random()<0.4: a,b=b,'0'
            if not a.startswith('%') and not b.startswith('%'): a='%'+random.choice(names)
            return f'  %k{k} = icmp {pr} {ty} {a}, {b}\n'
        extra+=cmp(1)+cmp(2)
        extra=extra.replace('%dsub','%dsub')
        t=f'{random.choice(["and","or"])} i1 %k1, %k2'; rty='i1'
    elif op=='icmp':
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
    elif op=='selcmp':
        pred=random.choice(['eq','ne','ult','ule','ugt','uge','slt','sle','sgt','sge'])
        l,r=operand(),operand()
        if not l.startswith('%'): l='%x'
        pool=[l,r,'%x','%y',str(random.choice(leaves(ty))),'0','-1']
        pa,pb=random.choice(pool),random.choice(pool)
        if random.random()<0.3:
            m=random.choice(['and','or'])
            c=random.choice([1,3,4,8,255])
            selpre=f'  %ma = {m} {ty} %x, {c}\n'
            pa=random.choice(['%ma','%x',pa]); pb=random.choice(['%ma','%x',pb])
            if random.random()<0.5: l,pred,r='%ma','eq' if random.random()<0.5 else 'ne','0'
        selpre+=f'  %kk = icmp {pred} {ty} {l}, {r}\n'
        t=f'select i1 %kk, {ty} {pa}, {ty} {pb}'
    elif op=='icall':
        name=random.choice(['smax','smin','umax','umin','uadd.sat','usub.sat','sadd.sat','ssub.sat','abs','cttz','ctlz','ctpop','bitreverse','bswap','scmp','ucmp','fshl','fshr','smul.fix'])
        if name=='bswap' and ty=='i8': name='ctpop'
        flag=random.choice(['true','false'])
        if name in ('abs','cttz','ctlz'): t=f'call {ty} @llvm.{name}.{ty}({ty} {a}, i1 {flag})'
        elif name in ('ctpop','bitreverse','bswap'): t=f'call {ty} @llvm.{name}.{ty}({ty} {a})'
        elif name in ('scmp','ucmp'): t=f'call i8 @llvm.{name}.i8.{ty}({ty} {a}, {ty} {b})'; rty='i8'
        elif name in ('fshl','fshr'): t=f'call {ty} @llvm.{name}.{ty}({ty} {a}, {ty} {b}, {ty} {random.choice(["%x","%y"]+[str(random.choice(leaves(ty)))])})'
        elif name=='smul.fix': t=f'call {ty} @llvm.smul.fix.{ty}({ty} {a}, {ty} {b}, i32 {random.choice([0,1,2,3])})'
        else: t=f'call {ty} @llvm.{name}.{ty}({ty} {a}, {ty} {b})'
    else:
        t=f'{op} {flags}{ty} {a}, {b}'
    # second layer: sometimes wrap an extension pair/compare of ext
    pre_text=''.join(f'  %{n} = {e}\n' for n,e in use)+mmdefs+extra+selpre
    sinkx=''.join(f'  store volatile i1 %k{k}, ptr @g_i1\n' for k in (1,2)) if extra else ''
    sink=sinkx+''.join(f'  store volatile {ty} %{n}, ptr @g_{ty}\n' for n,_ in use)+mmsink
    if op=='phi':
        body=f'  br i1 %c, label %p1, label %p2\np1:\n  br label %j\np2:\n  br label %j\nj:\n  %t = {t}\n  store volatile {rty} %t, ptr @g_{rty}\n  ret void\n'
        text=f'define void @f{k}({ty} %x, {ty} %y, i1 %c) {{\nb0:\n{pre_text}{sink}'+body+'}\n'
    else:
        text=f'define void @f{k}({ty} %x, {ty} %y, i1 %c) {{\nb0:\n{pre_text}{sink}  %t = {t}\n  store volatile {rty} %t, ptr @g_{rty}\n  ret void\n}}\n'
    cases.append(text)
hdr=''.join(f'@g_{t} = global {t} 0\n' for t in ['i1','i8','i16','i32','i64'])+''.join(f'declare {t} @llvm.{n}.{t}({t}, {t})\n' for t in ['i8','i16','i32'] for n in ('smax','smin','umax','umin'))
hdr+=''.join(f'declare {t} @llvm.{n}.{t}({t}, {t})\n' for t in ['i8','i16','i32'] for n in ('uadd.sat','usub.sat','sadd.sat','ssub.sat'))
hdr+=''.join(f'declare {t} @llvm.{n}.{t}({t}, i1)\n' for t in ['i8','i16','i32'] for n in ('abs','cttz','ctlz'))
hdr+=''.join(f'declare {t} @llvm.{n}.{t}({t})\n' for t in ['i8','i16','i32'] for n in ('ctpop','bitreverse'))
hdr+=''.join(f'declare {t} @llvm.bswap.{t}({t})\n' for t in ['i16','i32'])
hdr+=''.join(f'declare i8 @llvm.{n}.i8.{t}({t}, {t})\n' for t in ['i8','i16','i32'] for n in ('scmp','ucmp'))
hdr+=''.join(f'declare {t} @llvm.{n}.{t}({t}, {t}, {t})\n' for t in ['i8','i16','i32'] for n in ('fshl','fshr'))
hdr+=''.join(f'declare {t} @llvm.smul.fix.{t}({t}, {t}, i32 immarg)\n' for t in ['i8','i16','i32'])
open(sys.argv[3] if len(sys.argv)>3 else 'simplify_cases.ll','w').write(hdr+'\n'.join(cases))
