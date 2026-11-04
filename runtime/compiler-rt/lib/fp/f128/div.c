/* __divtf3, one quotient bit per step. */
#include "f128.h"

static inline sf128 div_core(sf128 sign, int ea, int eb, sf128 ma, sf128 mb)
{
    /* q = floor(ma * 2^116 / mb) and whether it was exact. */
    sf128 q = 0, r = ma;
    for (int i = 0; i < 117; i++) {
        q <<= 1;
        if (r >= mb) { r -= mb; q |= 1; }
        r <<= 1;
    }
    int inexact = r != 0;
    int e = ea - eb + 16382;
    if (q >> 116) { q = (q >> 1) | (q & 1); e++; }
    return SF_PACK(sign, e, q | inexact);
}

#include "../div.inc"

sf_quad SF_NAME(divtf3)(sf_quad a, sf_quad b) { return b2q(div_bits_128(q2b(a), q2b(b))); }

#include "../core_end.inc"
