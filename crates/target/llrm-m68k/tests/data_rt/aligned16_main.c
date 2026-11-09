/* exit = the number of 16 sp depths at which an aligned(16) local of f() was not on a 16-byte boundary (M68K-KNOWN-WRONG-ALIGN) */
static unsigned long seen;
void keep(long *p) { seen = (unsigned long)p; }
void f(void);
static long sys1(long n, long a) { register long d0 __asm__("d0") = n, d1 __asm__("d1") = a; __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1) : "memory"); return d0; }
__attribute__((noinline)) int run(int i) { volatile char *pad = __builtin_alloca(i * 4 + 4); pad[0] = 0; f(); return (seen & 15) != 0; }
int main(void) { int bad = 0; for (int i = 0; i < 16; i++) bad += run(i); return bad; }
void _start(void) { sys1(1, main()); for (;;) ; }
