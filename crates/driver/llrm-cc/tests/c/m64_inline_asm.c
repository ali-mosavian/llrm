int round_int(double x) { int v; __asm__("fistpl %0" : "=m"(v) : "t"(x) : "st"); return v; }
long long round_long(double x) { long long v; __asm__("fistpll %0" : "=m"(v) : "t"(x) : "st"); return v; }
void sincos_of(double *c, double *s, double a) {
    double co, si;
    __asm__("fsincos" : "=t"(co), "=u"(si) : "0"(a));
    *c = co;
    *s = si;
}
unsigned cpuid_a(unsigned leaf) {
    unsigned a, b, c, d;
    __asm__ volatile("cpuid" : "=a"(a), "=b"(b), "=c"(c), "=d"(d) : "a"(leaf));
    return a + b + c + d;
}
unsigned long long mul_hi(unsigned long long a, unsigned long long b) {
    unsigned long long lo, hi;
    __asm__("mulq %3" : "=a"(lo), "=d"(hi) : "0"(a), "c"(b));
    return hi ^ lo;
}
