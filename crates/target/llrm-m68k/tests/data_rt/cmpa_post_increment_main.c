/* exit 0 when k counts the elements above t as C does, for several arrays and bounds; else the number of wrong answers */
extern int k(int *p, int n, int t);
static long sys1(long n, long a) {
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1) : "memory");
    return d0;
}
static int expect(const int *p, int n, int t) { int s = 0; for (int i = 0; i < n; ++i) s += p[i] > t; return s; }
int main(void) {
    static int a[9] = { 5, -3, 9, 9, 0, 100, -100, 7, 7 };
    static const int ts[] = { -200, -3, 0, 6, 7, 9, 100, 200 };
    int bad = 0;
    for (int n = 1; n <= 9; ++n)
        for (unsigned j = 0; j < sizeof ts / sizeof ts[0]; ++j) bad += k(a, n, ts[j]) != expect(a, n, ts[j]);
    return bad;
}
void _start(void) { sys1(1, main()); for (;;) ; }
