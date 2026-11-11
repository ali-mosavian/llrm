int diamonds(void);
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
int main(void){ return diamonds() == 56 ? 0 : 1; }
void _start(void){ int r = main(); sys3(1, r, 0, 0); for(;;); }
