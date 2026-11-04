/* __divdf3. SF_DIV_NEWTON is the reciprocal form, SF_DIV64_UDIV two digits by a 64/32 divide (SF_UDIV64BY32), SF_DIV64_HW32 six by 32/32 divides, else one quotient bit per step. */
#include "f64.h"
#include "mul64.h"

/* hi:lo divided by d, with hi < d: the quotient in the low word and the remainder in the high word of the result, as one divide instruction gives them
 * (SF_UDIV64BY32 from the config). */
#ifndef SF_UDIV64BY32
static inline uint64_t sf_udiv64by32(uint32_t hi, uint32_t lo, uint32_t d)
{
    uint64_t n = ((uint64_t)hi << 32) | lo;
    uint32_t q = (uint32_t)(n / d);
    return q | (uint64_t)(uint32_t)(n - (uint64_t)q * d) << 32;
}
#define SF_UDIV64BY32 sf_udiv64by32
#endif

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
#elif defined(SF_DIV64_6502)
    /* The restoring loop below as a 6502 routine over fixed cells (config/6502/div64.s). */
    extern uint64_t sf_d64_ma, sf_d64_mb, sf_d64_q, sf_d64_r;
    extern void __sf_div64(void);
    sf_d64_ma = ma;
    sf_d64_mb = mb;
    __sf_div64();
    q = sf_d64_q, r = sf_d64_r;
    inexact = r != 0;
#elif defined(SF_DIV64_UDIV)
    /* The integer bit, then 56 fraction bits as two digits of 28. A digit is one 64/32 division of the shifted remainder by the top word of
     * mb (bit 31 set), off by at most 2 either way. The divide's own remainder r1 gives the exact one with a single 21x28-bit product:
     * r << 28 - d * mb = (r1 << 21) - d * (mb & 0x1FFFFF), as r << 7 = d * (mb >> 21) + r1. Then it is settled against the full divisor. */
    uint32_t mbh = (uint32_t)(mb >> 21), mbl = (uint32_t)mb & 0x1FFFFFu;
    r = ma;
    q = r >= mb;
    if (q) r -= mb;
#define DIV_STEP28() do { \
        uint64_t qr = SF_UDIV64BY32((uint32_t)(r >> 25), (uint32_t)(r << 7), mbh); \
        uint32_t d = (uint32_t)qr; \
        int64_t rem = (int64_t)(((qr >> 32) << 21) - (uint64_t)d * mbl); \
        while (rem < 0) { d--; rem += (int64_t)mb; } \
        while ((uint64_t)rem >= mb) { d++; rem -= (int64_t)mb; } \
        q = (q << 28) | d; r = (uint64_t)rem; } while (0)
    DIV_STEP28(); DIV_STEP28();
#undef DIV_STEP28
    inexact = r != 0;
#elif defined(SF_DIV64_HW32)
    /* The integer bit, then 56 fraction bits as five digits of 11 and one of 1. A digit is a 32/32 hardware division of the top words
     * of the shifted remainder by the top word of mb (21 bits), off by at most 2 either way, and settled against the full divisor.
     * d * mb may wrap past 2^64 on an overshoot; the remainder is right modulo 2^64 and small. */
    uint32_t mbh = (uint32_t)(mb >> 32);
    r = ma;
    q = r >= mb;
    if (q) r -= mb;
#define DIV_STEP(w) do { \
        uint64_t t = r << (w); \
        uint32_t d = (uint32_t)(t >> 32) / mbh; \
        int64_t rem = (int64_t)(t - (uint64_t)d * mb); \
        while (rem < 0) { d--; rem += (int64_t)mb; } \
        while ((uint64_t)rem >= mb) { d++; rem -= (int64_t)mb; } \
        q = (q << (w)) | d; r = (uint64_t)rem; } while (0)
    DIV_STEP(11); DIV_STEP(11); DIV_STEP(11); DIV_STEP(11); DIV_STEP(11); DIV_STEP(1);
#undef DIV_STEP
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
