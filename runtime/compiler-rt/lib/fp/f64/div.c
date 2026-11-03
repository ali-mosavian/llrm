/* __divdf3. SF_DIV_NEWTON is the reciprocal form, else one quotient bit per step. */
#include "f64.h"
#include "mul64.h"

static inline sf64 div_core(uint64_t sign, int ea, int eb, uint64_t ma, uint64_t mb)
{
    /* q = floor(ma * 2^56 / mb) and whether it was exact. */
    uint64_t q, r;
    int inexact;
#ifdef SF_DIV_NEWTON
    /* The f32 reciprocal (three 32-bit Newton steps from the linear start, ~2^-30 off) takes one 64-bit step to 2^-60; then
     * q ~ ma * x / 2^60 is off by at most 1 and the remainder, which only needs its low 64 bits, corrects it. x stands for 2^116 / mb,
     * a value in 2^63..2^64: mb = 2^52 makes the 32-bit part wrap (clamped). */
    uint64_t B = mb << 11, hi, lo;
    uint32_t b32 = (uint32_t)(B >> 32), h32, l32, x32 = 0x7504F333u - b32;
    for (int i = 0; i < 3; i++) {
        sf_mul32x32(x32, b32, &h32, &l32);
        uint32_t corr32 = 0u - h32;
        sf_mul32x32(x32, corr32, &h32, &l32);
        x32 = (h32 << 1) | (l32 >> 31);
    }
    if (x32 < 0x40000000u) x32 = 0xFFFFFFFFu;
    uint64_t x = (uint64_t)x32 << 32;
    mul64(x, B, &hi, &lo);
    mul64(x, 0 - hi, &hi, &lo);
    x = (hi << 1) | (lo >> 63);
    mul64(ma << 11, x, &hi, &lo);
    q = hi >> 7;
    uint32_t ql = (uint32_t)q, qh = (uint32_t)(q >> 32), bl = (uint32_t)mb, bh = (uint32_t)(mb >> 32);
    sf_mul32x32(ql, bl, &h32, &l32);
    r = (ma << 56) - (((uint64_t)(h32 + ql * bh + qh * bl) << 32) | l32);
    while ((int64_t)r < 0) { q--; r += mb; }
    while (r >= mb) { q++; r -= mb; }
    inexact = r != 0;
#else
    /* One quotient bit per step. */
    q = 0, r = ma;
    for (int i = 0; i < 57; i++) {
        q <<= 1;
        if (r >= mb) { r -= mb; q |= 1; }
        r <<= 1;
    }
    inexact = r != 0;
#endif
    int e = ea - eb + 1022;
    if (q >> 56) { q = (q >> 1) | (q & 1); e++; }
    return SF_PACK(sign, e, q | inexact);
}

#include "../div.inc"

sf_double SF_NAME(divdf3)(sf_double a, sf_double b) { return b2d(div_bits_64(d2b(a), d2b(b))); }

#include "../core_end.inc"
