extern int main(void);
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
void report(long v){ char b[24],t[24]; int n=0,k=0; unsigned long u=v<0?-(unsigned long)v:(unsigned long)v; do{t[k++]='0'+u%10;u/=10;}while(u); if(v<0)b[n++]='-'; while(k)b[n++]=t[--k]; b[n++]='\n'; sys3(4,1,(long)b,n);}
void _start(void){ int r=main(); sys3(1,r,0,0); for(;;); }
typedef unsigned long u32; typedef unsigned long long u64;
u32 __udivmodsi(u32 n,u32 d,u32*r){u32 q=0,rem=0;for(int i=31;i>=0;i--){rem=(rem<<1)|((n>>i)&1);if(rem>=d){rem-=d;q|=(u32)1<<i;}}*r=rem;return q;}
u32 __mulsi3(u32 a,u32 b){u32 p=0;while(b){if(b&1)p+=a;a<<=1;b>>=1;}return p;}
u32 __udivsi3(u32 n,u32 d){u32 r;return __udivmodsi(n,d,&r);}
u32 __umodsi3(u32 n,u32 d){u32 r;__udivmodsi(n,d,&r);return r;}
long __divsi3(long n,long d){int s=(n<0)^(d<0);u32 r,q=__udivmodsi(n<0?-n:n,d<0?-d:d,&r);return s?-(long)q:(long)q;}
long __modsi3(long n,long d){u32 r;__udivmodsi(n<0?-n:n,d<0?-d:d,&r);return n<0?-(long)r:(long)r;}
void*memcpy(void*d,const void*s,unsigned long n){char*a=d;const char*b=s;while(n--)*a++=*b++;return d;}
void*memmove(void*d,const void*s,unsigned long n){char*a=d;const char*b=s;if(a<b)while(n--)*a++=*b++;else{a+=n;b+=n;while(n--)*--a=*--b;}return d;}
void*memset(void*d,int c,unsigned long n){char*a=d;while(n--)*a++=c;return d;}
