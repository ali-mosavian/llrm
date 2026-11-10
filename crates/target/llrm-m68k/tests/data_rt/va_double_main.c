/* Exit 0 when sumhi(3, 1.5, .25, 1.5) is right through call_a and call_b; 1 or 2 name the call that read a wrong double. */
extern int sumhi(int n, ...);
extern int call_a(double a, double b, double c);
extern int call_b(double a, double b, double c);
#ifndef OURS_CALLERS
__attribute__((noinline)) int call_a(double a, double b, double c) { return sumhi(3, a, b, c); }
__attribute__((noinline)) int call_b(double a, double b, double c) { volatile int pad = 1; return sumhi(3, a, b, c) + pad - 1; }
#endif
int main(void) {
    union { double d; unsigned long w[2]; } x = { 1.5 }, y = { .25 };
    int want = (int)(x.w[0] + y.w[0] + x.w[0]);
    return (call_a(1.5, .25, 1.5) != want) | (call_b(1.5, .25, 1.5) != want) << 1;
}
static long sys1(long n, long a) {
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1) : "memory");
    return d0;
}
void _start(void) { sys1(1, main()); for (;;) ; }
