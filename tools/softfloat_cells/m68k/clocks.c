/* One f64 operation over the vectors of runtime/compiler-rt/tests/gen_vectors64.c, for `cycles --exe`: harness = h_*, main, _start; everything else executed is library.
 * OP: 0 add, 1 sub, 2 mul, 3 div, 4 lt, 5 floatsidf, 6 floatdidf, 7 fixdfsi, 8 extendsfdf2, 9 truncdfsf2. Results are summed, not checked (outputs are compared by the library tests). */
#include <stdint.h>
#include "vectors64.h"
double __adddf3(double, double), __subdf3(double, double), __muldf3(double, double), __divdf3(double, double), __floatsidf(int), __floatdidf(long long), __extendsfdf2(float);
int __ltdf2(double, double), __fixdfsi(double);
float __truncdfsf2(double);
static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static double h_mk(uint32_t hi, uint32_t lo) { union { uint32_t w[2]; double d; } u; u.w[0] = hi; u.w[1] = lo; return u.d; }
static uint32_t h_lo(double d) { union { uint32_t w[2]; double d; } u; u.d = d; return u.w[0] ^ u.w[1]; }
static float h_f(uint32_t b) { union { uint32_t b; float f; } u; u.b = b; return u.f; }
static uint32_t h_b(float f) { union { uint32_t b; float f; } u; u.f = f; return u.b; }
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
#elif OP == 3
        acc += h_lo(__divdf3(x, y));
#elif OP == 4
        acc += __ltdf2(x, y);
#elif OP == 5
        acc += h_lo(__floatsidf((int)(V[i][1] >> (V[i][3] & 31))));
#elif OP == 6
        acc += h_lo(__floatdidf((long long)(((unsigned long long)V[i][0] << 32 | V[i][1]) >> (V[i][3] & 63))));
#elif OP == 7
        acc += __fixdfsi(h_mk((V[i][0] & 0x800FFFFFu) | ((0x3E0u + (V[i][1] & 63)) << 20), V[i][1]));
#elif OP == 8
        acc += h_lo(__extendsfdf2(h_f(V[i][0])));
#elif OP == 9
        acc += h_b(__truncdfsf2(x));
#else
        acc += h_lo(x) ^ h_lo(y); /* null: no library call */
#endif
    }
    sys3(1, acc == 0xDEADBEEFu, 0, 0);
    return 0;
}
void _start(void) { main(); for (;;) ; }
