"""A 1 KB local buffer with no spills counted 1,000+ stack ops (diag.c 0x40c); a spill-free function must count 0 spill slots."""
import os,subprocess,tempfile,sys,re
src=open(os.path.join(os.path.dirname(__file__),'spillcensus.py')).read().split("files=sorted")[0]
os.environ.setdefault('QB_TREE','/nonexistent'); sys.argv=['x','m64','Os','x']; ns={'__file__':os.path.join(os.path.dirname(__file__),'spillcensus.py')}; exec(src,ns)
def obj(c):
    d=tempfile.mkdtemp(); o=d+'/o.o'; subprocess.run(['gcc','-Os','-m64','-fno-pic','-fno-stack-protector','-fcf-protection=none','-c','-x','c','-','-o',o],input=c,text=True,check=True); return o
def test_buffer_is_not_a_spill():
    o=obj('void use(char*); int f(int n){char b[1024]; b[0]=n; b[8]=n+1; b[16]=n+2; b[24]=n+3; use(b); return b[0]+b[8]+b[16]+b[24];}')
    assert ns['count'](o)==(0,0)

def test_real_spill_counts():
    o=obj('void a(void); int h(int*p){int v0=p[0],v1=p[1],v2=p[2],v3=p[3],v4=p[4],v5=p[5],v6=p[6],v7=p[7],v8=p[8],v9=p[9];a();return v0*v1+v2*v3+v4*v5+v6*v7+v8*v9+(v0^v9)+(v1^v8)+(v2^v7);}')
    assert sum(ns['count'](o))>0
