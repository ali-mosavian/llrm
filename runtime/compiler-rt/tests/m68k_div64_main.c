/* 64-bit / and % through the libcalls (__udivdi3 __umoddi3 __divdi3 __moddi3) against vectors.h; exit code = mismatches. */
#include <stdint.h>
#include "vectors.h"
static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
#ifdef VIA_CALLS /* the libcall sites come from m68k_div64_calls.c, built by llrm-cc */
uint64_t f_udiv(uint64_t, uint64_t), f_umod(uint64_t, uint64_t);
int64_t f_div(int64_t, int64_t), f_mod(int64_t, int64_t);
#define UDIV(n, d) f_udiv(n, d)
#define UMOD(n, d) f_umod(n, d)
#define SDIV(n, d) f_div(n, d)
#define SMOD(n, d) f_mod(n, d)
#else
#define UDIV(n, d) ((n) / (d))
#define UMOD(n, d) ((n) % (d))
#define SDIV(n, d) ((n) / (d))
#define SMOD(n, d) ((n) % (d))
#endif
#define W(h, l) (((uint64_t)(h) << 32) | (l))
int main(void)
{
    long bad[4] = {0, 0, 0, 0};
    for (unsigned i = 0; i < sizeof V / sizeof *V; i++) {
        const unsigned *v = V[i];
        uint64_t n = W(v[0], v[1]), d = W(v[2], v[3]);
        volatile uint64_t vn = n, vd = d; /* keep the operands out of the folder */
        n = vn; d = vd;
        bad[0] += (uint64_t)UDIV(n, d) != W(v[4], v[5]);
        bad[1] += (uint64_t)UMOD(n, d) != W(v[6], v[7]);
        bad[2] += (uint64_t)SDIV((int64_t)n, (int64_t)d) != W(v[8], v[9]);
        bad[3] += (uint64_t)SMOD((int64_t)n, (int64_t)d) != W(v[10], v[11]);
    }
    long t = bad[0] + bad[1] + bad[2] + bad[3];
    sys3(1, t > 255 ? 255 : t, 0, 0);
    return 0;
}
void _start(void) { main(); for (;;) ; }
