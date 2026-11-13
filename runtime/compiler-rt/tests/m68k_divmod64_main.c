/* 64-bit quotient and remainder in one call (__udivmoddi4 __divmoddi4) against vectors.h; exit code = mismatches. */
#include <stdint.h>
#include "vectors.h"
static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
uint64_t __udivmoddi4(uint64_t, uint64_t, uint64_t *);
int64_t __divmoddi4(int64_t, int64_t, int64_t *);
#define W(h, l) (((uint64_t)(h) << 32) | (l))
int main(void)
{
    long bad = 0;
    for (unsigned i = 0; i < sizeof V / sizeof *V; i++) {
        const unsigned *v = V[i];
        volatile uint64_t vn = W(v[0], v[1]), vd = W(v[2], v[3]);
        uint64_t n = vn, d = vd, ur = 0xdeadbeefdeadbeefull;
        int64_t sr = 0x0123456789abcdefll;
        uint64_t uq = __udivmoddi4(n, d, &ur);
        int64_t sq = __divmoddi4((int64_t)n, (int64_t)d, &sr);
        bad += uq != W(v[4], v[5]) || ur != W(v[6], v[7]);
        bad += (uint64_t)sq != W(v[8], v[9]) || (uint64_t)sr != W(v[10], v[11]);
    }
    sys3(1, bad > 255 ? 255 : bad, 0, 0);
    return 0;
}
