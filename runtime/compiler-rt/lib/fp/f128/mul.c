/* __multf3. */
#include "f128.h"
#include "../f64/mul64.h"

/* 113x113 -> 226 as hi:lo, from four 64x64 products: no 128-bit multiply (a target without one would call __multi3 for each). */
static inline void mul128(sf128 a, sf128 b, sf128 *hi, sf128 *lo)
{
    uint64_t a0 = (uint64_t)a, a1 = (uint64_t)(a >> 64), b0 = (uint64_t)b, b1 = (uint64_t)(b >> 64);
    uint64_t h, l;
    mul64(a0, b0, &h, &l); sf128 p00 = ((sf128)h << 64) | l;
    mul64(a0, b1, &h, &l); sf128 p01 = ((sf128)h << 64) | l;
    mul64(a1, b0, &h, &l); sf128 p10 = ((sf128)h << 64) | l;
    mul64(a1, b1, &h, &l); sf128 p11 = ((sf128)h << 64) | l;
    sf128 t = p01 << 64, r = p00 + t, carry = r < t;
    t = p10 << 64;
    r += t;
    carry += r < t;
    *lo = r;
    *hi = p11 + (p01 >> 64) + (p10 >> 64) + carry;
}

static inline sf128 mul_core(sf128 sign, int ea, int eb, sf128 ma, sf128 mb)
{
    sf128 hi, lo;
    mul128(ma, mb, &hi, &lo);
    /* The product has its top bit at 225 or 224; keep 116 bits and fold the rest. */
    int e = ea + eb - 16383, sh = 109;
    if (hi >> 97) { sh = 110; e++; }
    sf128 m = (hi << (128 - sh)) | (lo >> sh) | ((lo << (128 - sh)) != 0);
    return SF_PACK(sign, e, m);
}

#include "../mul.inc"

sf_quad SF_NAME(multf3)(sf_quad a, sf_quad b) { return b2q(mul_bits_128(q2b(a), q2b(b))); }

#include "../core_end.inc"
