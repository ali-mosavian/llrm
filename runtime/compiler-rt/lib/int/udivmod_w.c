/* Restoring 64/64 divide; division by zero gives all ones and the remainder is the dividend. */
#include "i64.h"

/* Worker takes and returns 32-bit scalars: struct-by-value is not a stable ABI across targets. */
void SFB_NAME(udivmod_w)(uint32_t nlo, uint32_t nhi, uint32_t dlo, uint32_t dhi, uint32_t out[4])
{
    pair n = { nlo, nhi }, d = { dlo, dhi }, q = { 0, 0 };
    if (!(d.lo | d.hi)) { q.lo = q.hi = ~0u; }
    else if (ge(n, d)) {
        int s = clz(d) - clz(n);
        d = shl(d, s);
        for (int i = 0; i <= s; i++) {
            q = shl(q, 1);
            if (ge(n, d)) { n = sub(n, d); q.lo |= 1; }
            d = shr1(d);
        }
    }
    out[0] = q.lo; out[1] = q.hi; out[2] = n.lo; out[3] = n.hi;
}
