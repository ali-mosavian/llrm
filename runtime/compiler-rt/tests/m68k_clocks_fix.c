/* __fixsfsi (OP 0) and __fixunssfsi (OP 1) over the finite in-range vectors of gen_vectors.c, for tools like the m68k `cycles` example: the harness is the functions named h_*,
 * main and _start; everything else executed is library. */
#include <stdint.h>
#include "vectors.h"

int32_t __fixsfsi(float);
uint32_t __fixunssfsi(float);

static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static float h_b2f(uint32_t b) { union { uint32_t b; float f; } u; u.b = b; return u.f; }

int main(void)
{
    uint32_t acc = 0;
    for (int i = 0; i < NV; i++) {
        uint32_t a = V[i][0];
        if (((a >> 23) & 0xFF) >= 158) continue; /* NaN or >= 2^31: undefined in C */
#if OP == 0
        acc += (uint32_t)__fixsfsi(h_b2f(a));
#else
        acc += __fixunssfsi(h_b2f(a));
#endif
    }
    sys3(1, acc == 0xDEADBEEFu, 0, 0);
    return 0;
}
void _start(void) { main(); for (;;) ; }
