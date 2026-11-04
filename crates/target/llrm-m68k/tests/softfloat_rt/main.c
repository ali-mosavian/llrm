double dadd(double,double),dsub(double,double),dmul(double,double),ddiv(double,double),ext(float),i2d(int),neg(double),sel(double,double,int),dsum(const double*,int),mix(double,float,int);
float fadd2(float,float),fmul2(float,float),trunc2(double),i2f(int);
int d2i(double),flt(double,double),feq(double,double),fge(float,float);
void st(double*,double);
static int bad;
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
static int same(const void*a,const void*b,int n){const unsigned char*x=a,*y=b;while(n--)if(*x++!=*y++)return 0;return 1;}
static void say(const char*s){int n=0;while(s[n])n++;sys3(4,1,(long)s,n);}
#define CHK(name,got,want) do{ if(!same(&(got),&(want),sizeof(got))){bad++;say("MISMATCH " name "\n");} }while(0)
int main(void){
  double a[]={0.0,1.5,-2.25,3.14159,1e10,-7.0e-3,100.0};
  float  f[]={0.5f,-1.25f,3.0f,1e5f,7.7f};
  for(int i=0;i<7;i++)for(int j=0;j<7;j++){
    double x=a[i],y=a[j],g,w;
    g=dadd(x,y); w=x+y; CHK("dadd",g,w);
    g=dsub(x,y); w=x-y; CHK("dsub",g,w);
    g=dmul(x,y); w=x*y; CHK("dmul",g,w);
    if(y!=0){g=ddiv(x,y); w=x/y; CHK("ddiv",g,w);}
    int c=flt(x,y), cw=x<y; CHK("flt",c,cw);
    c=feq(x,y); cw=x==y; CHK("feq",c,cw);
    g=sel(x,y,i&1); w=(i&1)?y:x; CHK("sel",g,w);
  }
  for(int i=0;i<5;i++)for(int j=0;j<5;j++){
    float x=f[i],y=f[j],g,w;
    g=fadd2(x,y); w=x+y; CHK("fadd2",g,w);
    g=fmul2(x,y); w=x*y; CHK("fmul2",g,w);
    int c=fge(x,y), cw=x>=y; CHK("fge",c,cw);
  }
  for(int i=0;i<7;i++){ double g,w; g=neg(a[i]); w=-a[i]; CHK("neg",g,w);
    float tg=trunc2(a[i]), tw=(float)a[i]; CHK("trunc2",tg,tw);
    int ig=d2i(a[i]), iw=(int)a[i]; CHK("d2i",ig,iw); }
  for(int i=0;i<5;i++){ double g=ext(f[i]), w=f[i]; CHK("ext",g,w);
    for(int j=0;j<7;j++){ g=mix(a[j],f[i],i-j); w=a[j]*(double)f[i]+(double)(i-j); CHK("mix",g,w);} }
  for(int n=-3;n<=9;n+=3){ double g=i2d(n), w=n; CHK("i2d",g,w); float fg=i2f(n), fw=n; CHK("i2f",fg,fw); }
  double s=dsum(a,7), sw=0; for(int i=0;i<7;i++)sw+=a[i]; CHK("dsum",s,sw);
  double box=0; st(&box,2.5); double want=2.5; CHK("st",box,want);
  if(!bad) say("ALL OK\n");
  return bad!=0;
}
void _start(void){ int r=main(); sys3(1,r,0,0); for(;;); }
