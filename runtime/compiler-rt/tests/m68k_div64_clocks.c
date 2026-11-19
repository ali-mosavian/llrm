/* One 64-bit libcall per vector (OP 0 udiv, 1 umod, 2 div, 3 mod; 99 none) over vectors.h, zero divisors skipped; returns a checksum so nothing folds. */
#include <stdint.h>
#include "vectors.h"
#define W(h, l) (((uint64_t)(h) << 32) | (l))
volatile uint64_t sink;
int main(void)
{
    uint64_t acc = 0;
    for (unsigned i = NEDGE; i < NEDGE + NRND; i++) {
        const unsigned *v = V[i];
        volatile uint64_t vn = W(v[0], v[1]), vd = W(v[2], v[3]);
        uint64_t n = vn, d = vd;
        if (!d) continue;
#ifdef SIGNMIX /* half the operands negative */
        if (i & 1) n = -n;
        if (i & 2) d = -d;
#endif
#ifdef CLASS /* 1: d < 2^32, n.hi < d (one divide); 2: d < 2^32, n.hi >= d (two); 3: d >= 2^32 */
        if ((d >> 32 ? 3 : (n >> 32) < d ? 1 : 2) != CLASS) continue;
#endif
#if OP == 0
        acc += n / d;
#elif OP == 1
        acc += n % d;
#elif OP == 2
        acc += (uint64_t)((int64_t)n / (int64_t)d);
#elif OP == 3
        acc += (uint64_t)((int64_t)n % (int64_t)d);
#else
        acc += n ^ d;
#endif
    }
    sink = acc;
    return 0;
}
void _start(void) { main(); __asm__ volatile("moveq #1,%%d0; moveq #0,%%d1; trap #0" ::: "d0", "d1"); for (;;) ; }
