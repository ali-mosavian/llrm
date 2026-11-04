/* One operation over the vectors of gen_vectors.c (or gen_vectors64.c with -DF64: OP 0..3 only), for pricing: OP 0 add, 1 sub, 2 mul, 3 div, 4 cmp. The harness is the functions
 * named h_*, main and _start; everything else executed is library. Build with m68k-run-c.sh-style flags and -DOP=n. */
#include <stdint.h>
#ifdef F64
#include "vectors64.h"
#else
#include "vectors.h"
#endif

double __adddf3(double, double), __subdf3(double, double), __muldf3(double, double), __divdf3(double, double);
float __addsf3(float, float), __subsf3(float, float), __mulsf3(float, float), __divsf3(float, float);
int __cmpsf2(float, float), __unordsf2(float, float);

static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static float h_b2f(uint32_t b) { union { uint32_t b; float f; } u; u.b = b; return u.f; }
static uint32_t h_f2b(float f) { union { uint32_t b; float f; } u; u.f = f; return u.b; }

#ifdef F64
static double h_mk(uint32_t hi, uint32_t lo) { union { uint32_t w[2]; double d; } u; u.w[0] = hi; u.w[1] = lo; return u.d; }
static uint32_t h_lo(double d) { union { uint32_t w[2]; double d; } u; u.d = d; return u.w[0] ^ u.w[1]; }
int main(void)
{
    uint32_t acc = 0;
    for (int i = 0; i < NV; i++) {
        double x = h_mk(V[i][0], V[i][1]), y = h_mk(V[i][2], V[i][3]);
#if OP == 0
        acc += h_lo(__adddf3(x, y));
#elif OP == 1
        acc += h_lo(__subdf3(x, y));
#elif OP == 2
        acc += h_lo(__muldf3(x, y));
#else
        acc += h_lo(__divdf3(x, y));
#endif
    }
    sys3(1, acc == 0xDEADBEEFu, 0, 0);
    return 0;
}
#else
int main(void)
{
    uint32_t acc = 0;
    for (int i = 0; i < NV; i++) {
        float x = h_b2f(V[i][0]), y = h_b2f(V[i][1]);
#if OP == 0
        acc += h_f2b(__addsf3(x, y));
#elif OP == 1
        acc += h_f2b(__subsf3(x, y));
#elif OP == 2
        acc += h_f2b(__mulsf3(x, y));
#elif OP == 3
        acc += h_f2b(__divsf3(x, y));
#else
        acc += __unordsf2(x, y) ? 2 : __cmpsf2(x, y);
#endif
    }
    sys3(1, acc == 0xDEADBEEFu, 0, 0);
    return 0;
}
#endif
void _start(void) { main(); for (;;) ; }
