/* exit 0 when f(k) stored 9 at buf[500 + k] for each k tried (g sees buf); else the number of wrong k */
extern void f(int k);
char buf[1024];
static int bad, current;
void g(const char *buf) { if (buf[500 + current] != 9) ++bad; }
static long sys1(long n, long a) {
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1) : "memory");
    return d0;
}
int main(void) {
    static const int ks[] = { 0, 1, 77, 255, 399, 500 };
    for (unsigned i = 0; i < sizeof ks / sizeof ks[0]; ++i) { current = ks[i]; f(current); }
    return bad;
}
void _start(void) { sys1(1, main()); for (;;) ; }
