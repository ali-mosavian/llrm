"""z80_funcsize.py A.s B.s ...: bytes of each function in z80 assembly files, from the sdasz80 symbol table.
The last function ends at the last listed address, so it is one byte short."""
import subprocess,sys,re,os,shutil
def sizes(s):
    d=os.path.dirname(os.path.abspath(s)); b=os.path.basename(s)[:-2]
    subprocess.run(["sdasz80","-g","-los",s],cwd=d,check=True,capture_output=True)
    syms=[]
    for m in re.finditer(r"\b0 (_\w+)\s+([0-9A-F]{8}) G?R",open(f"{d}/{b}.sym").read()):
        syms.append((int(m.group(2),16),m.group(1)))
    # end of area
    end=max(int(m.group(1),16)+0 for m in [re.match(r'\s*([0-9A-F]{4,8})',l) for l in open(f'{d}/{b}.lst') if re.match(r'\s*[0-9A-F]{4,8} ',l)] if m)
    syms.sort()
    out={}
    for (a,n),(a2,_) in zip(syms,syms[1:]+[(end,None)]):
        out[n]=(a2-a) if a2 is not None else None
    return out
r={f:sizes(f+".s") for f in sys.argv[1:]}
names=sorted(set().union(*[set(v) for v in r.values()]))
print("fn",*sys.argv[1:])
for n in names: print(n,*[r[f].get(n) for f in sys.argv[1:]])
