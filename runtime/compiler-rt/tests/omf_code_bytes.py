# usage: omf_code_bytes.py OBJ... -> code segment bytes of each OMF object (llrm-cc x86_16-wat has one segment per file, no function sections)
import sys,struct
def lnames(d):
    out=[];i=0
    while i<len(d):
        n=d[i]; out.append(d[i+1:i+1+n].decode()); i+=1+n
    return out
def sizes(path):
    b=open(path,'rb').read(); i=0; ln=[]; segs=[]
    while i<len(b):
        t=b[i]; ln_=struct.unpack('<H',b[i+1:i+3])[0]; d=b[i+3:i+3+ln_-1]; i+=3+ln_
        if t==0x96: ln+=lnames(d)
        if t in (0x98,0x99):
            a=d[0]; p=1
            if (a>>5)==0: p+=3
            if t==0x98: l=struct.unpack('<H',d[p:p+2])[0]; p+=2
            else: l=struct.unpack('<I',d[p:p+4])[0]; p+=4
            def idx():
                nonlocal p
                v=d[p]; p+=1
                if v&0x80: v=((v&0x7f)<<8)|d[p]; p+=1
                return v
            s=idx(); c=idx(); segs.append((ln[s-1],ln[c-1],l))
    return segs
for f in sys.argv[1:]:
    print(sum(l for _, c, l in sizes(f) if c == 'CODE'))
