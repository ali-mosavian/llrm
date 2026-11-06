/* Restoring 64/64 divide; division by zero gives all ones and the remainder is the dividend. */
#include "i64.h"

/* Worker takes and returns 32-bit scalars: struct-by-value is not a stable ABI across targets. */
void SFB_NAME(udivmod_w)(uint32_t nlo, uint32_t nhi, uint32_t dlo, uint32_t dhi, uint32_t out[4])
{
    pair n = { nlo, nhi }, d = { dlo, dhi }, q = { 0, 0 };
    if (!(d.lo | d.hi)) { q.lo = q.hi = ~0u; }
#ifdef SF_DIV64_NARROW /* a target with a cheap 32-bit divide and no wide registers: 6502 -O2 __udivdi3 8786 -> 6424 cycles on 48 mixed pairs, +368 B */
    else if (!d.hi) { /* one-word divisor: the remainder is one word; the high word divides only when it can */
        uint32_t r = 0, nl = n.lo, ql = 0;
        int steps = 32;
        if (n.hi >= d.lo) { q.hi = n.hi / d.lo; r = n.hi % d.lo; }
        else if (n.hi) r = n.hi;
        else if (nl) { int z = sf_clz32(nl); nl <<= z; steps = 32 - z; } /* the leading zero bits of a one-word dividend need no step */
        else steps = 0;
        for (int i = 0; i < steps; i++) {
            uint32_t c = r >> 31;
            r = (r << 1) | (nl >> 31); nl <<= 1; ql <<= 1;
            if (c || r >= d.lo) { r -= d.lo; ql |= 1; }
        }
        q.lo = ql; n.lo = r; n.hi = 0;
    }
#endif
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
