int f(void), g(void), h(void);
extern char to[70016], from[70016];
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
void _start(void){ long r = f(); int ok = g() == (long)&to[70000] && h() == (long)&from[69999]; sys3(1, ok ? r : 99, 0, 0); for(;;); }
