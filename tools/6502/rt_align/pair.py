"""One __mulsi3 call linked at pads 0..255, with a nop control for the call (cancels the startup BSS-zero loop). RT_S=mos_rt.s
python3 pair.py writes pair.json: (pad, __mulsi3 address, clocks, control clocks)."""
import re, subprocess, os, sys
RT=open(os.environ["RT_S"]).read()
# minimal runtime: __mulsi3 + copy_st_to_sa + the BSS block, cut from the real mos_rt.s
def blocks():
    out=[];cur=None
    for l in RT.splitlines():
        m=re.match(r"^([A-Za-z_]\w*):",l)
        if m: cur=[m.group(1),l+"\n"];out.append(cur)
        elif cur: cur[1]+=l+"\n"
    return {n:t for n,t in out}
B=blocks()
bss_start=RT.index('.segment "BSS"'); bss_end=RT.index('.segment "CODE"',bss_start)
bss=RT[bss_start:bss_end]
prog='''.export _main
_main:
  lda #$79
  sta __sa
  lda #$56
  sta __sa+1
  lda #$34
  sta __sa+2
  lda #$12
  sta __sa+3
  lda #$EF
  sta __sb
  lda #$BE
  sta __sb+1
  lda #$AD
  sta __sb+2
  lda #$DE
  sta __sb+3
  jsr __mulsi3
  lda #0
  jmp $FFF9
'''
def build(pad, call=True):
    t=".setcpu \"6502\"\n"+"".join(f"rc{i} = ${0x80+i:X}\n" for i in range(8))
    rt="".join(l+"\n" for l in (B["__mulsi3"]+B["copy_st_to_sa"]).splitlines() if not l.startswith(".export __mulsi3_fast")); rt=".export __mulsi3\n"+rt
    t+=".import __sa,__sb,__st\n" if False else ""
    prog_=prog if call else prog.replace('jsr __mulsi3','nop\n  nop\n  nop')
    t+=f'.segment "CODE"\n.res {pad}\n'+prog_+rt+"\n"+bss+'.segment "CODE"\n'
    open("f.s","w").write(t)
    k=subprocess.run(["cl65","-t","sim6502","-m","x.map","-Ln","x.lbl","-o","x.bin","f.s"],capture_output=True,text=True)
    if k.returncode: print(k.stderr[-400:]); sys.exit(1)
    lbl=open("x.lbl").read()
    addr=int(re.search(r"al ([0-9A-F]+) \.__mulsi3\b",lbl).group(1),16)
    r=subprocess.run(["sim65","-c","-x","4000000000","x.bin"],capture_output=True)
    return addr,int(re.search(rb"(\d+) cycles",r.stdout+r.stderr).group(1))
if __name__=="__main__":
    res=[]
    for pad in range(256):
        a,c=build(pad); a2,c0=build(pad,False); assert a==a2; res.append((pad,a,c,c0))
    import json; json.dump(res,open("pair.json","w"))
    for pad,a,c in res[:3]: print(pad,hex(a),c)
