/* IEEE-754 binary64, round to nearest even, denormals, inf and NaN; the binary32 core (sf32.c) at twice the width, from the same width-generic
 * sf_codec.inc and sf_arith.inc. Working form: 56-bit mantissa (implicit one at bit 55, three guard bits). Needs a 64-bit type. */
#include "softfloat.h"
#include "sf_helpers.h"

#define LOW32 0xFFFFFFFFull

#define SF_N(n) n##_64
#define SF_T uint64_t
#define SF_S int64_t
#define SF_W 64
#define SF_MB 52
#define SF_EXPMAX 0x7FF
#define SF_QNAN 0x7FF8000000000000ull
#define sf_clz sf_clz64
#define sf_shr_sticky sf_shr_sticky64
#define SF_ROUND3 sf_round3_64
#define SF_PACK SF_N(pack_any)
#include "sf_codec.inc"

/* 64x64 -> 128 as hi:lo from 32x32 partial products. */
static void mul64(uint64_t a, uint64_t b, uint64_t *hi, uint64_t *lo)
{
    uint32_t h, l;
    uint64_t ll, lh, hl, hh, mid;
    sf_mul32x32((uint32_t)a, (uint32_t)b, &h, &l); ll = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)a, (uint32_t)(b >> 32), &h, &l); lh = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)(a >> 32), (uint32_t)b, &h, &l); hl = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)(a >> 32), (uint32_t)(b >> 32), &h, &l); hh = (uint64_t)h << 32 | l;
    mid = (ll >> 32) + (lh & LOW32) + (hl & LOW32);
    *lo = (ll & LOW32) | (mid << 32);
    *hi = hh + (lh >> 32) + (hl >> 32) + (mid >> 32);
}

static inline sf64 mul_core(uint64_t sign, int ea, int eb, uint64_t ma, uint64_t mb)
{
    uint64_t hi, lo;
    mul64(ma, mb, &hi, &lo);
    /* The product has its top bit at 105 or 104; keep 56 bits and fold the rest. */
    int e = ea + eb - 1023, sh = 49;
    if (hi >> 41) { sh = 50; e++; }
    uint64_t m = (hi << (64 - sh)) | (lo >> sh) | ((lo << (64 - sh)) != 0);
    return pack_any_64(sign, e, m);
}

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
    return pack_any_64(sign, e, q | inexact);
}

#include "sf_arith.inc"

/* |a| truncated; *big is set from 2^bits up, where the result saturates. */
static uint64_t magnitude(sf64 a, int bits, int *big)
{
    int e = (int)((a >> 52) & 0x7FF) - 1023;
    *big = 0;
    if (e < 0) return 0;
    if (e >= bits) { *big = 1; return 0; }
    uint64_t m = (a & (SF_HID - 1)) | SF_HID;
    return e >= 52 ? m << (e - 52) : m >> (52 - e);
}

static int64_t to_signed(sf64 a, int bits)
{
    int big;
    uint64_t m = magnitude(a, bits, &big), top = 1ull << (bits - 1);
    if (is_nan_64(a)) return 0;
    if (a & SF_SIGN) return (big || m > top) ? (int64_t)(0ull - top) : (int64_t)(0ull - m);
    return (big || m > top - 1) ? (int64_t)(top - 1) : (int64_t)m;
}

static uint64_t to_unsigned(sf64 a, int bits)
{
    int big;
    uint64_t m = magnitude(a, bits, &big);
    if (is_nan_64(a) || (a & SF_SIGN)) return 0;
    return big ? (bits == 64 ? UINT64_MAX : (1ull << bits) - 1) : m;
}

static sf64 from_mag(uint64_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint64_t m = n > 55 ? sf_shr_sticky64(u, n - 55) : u << (55 - n);
    return pack_any_64(sign, n + 1023, m);
}

sf_double SF_NAME(adddf3)(sf_double a, sf_double b) { return b2d(add_bits_64(d2b(a), d2b(b))); }
sf_double SF_NAME(subdf3)(sf_double a, sf_double b) { return b2d(add_bits_64(d2b(a), d2b(b) ^ SF_SIGN)); }
sf_double SF_NAME(muldf3)(sf_double a, sf_double b) { return b2d(mul_bits_64(d2b(a), d2b(b))); }
sf_double SF_NAME(divdf3)(sf_double a, sf_double b) { return b2d(div_bits_64(d2b(a), d2b(b))); }
sf_double SF_NAME(negdf2)(sf_double a) { return b2d(d2b(a) ^ SF_SIGN); }

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. */
int SF_NAME(cmpdf2)(sf_double a, sf_double b) { return cmp_bits_64(d2b(a), d2b(b), 1); }
int SF_NAME(eqdf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(nedf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(ltdf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(ledf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(gedf2)(sf_double a, sf_double b) { return cmp_bits_64(d2b(a), d2b(b), -1); }
int SF_NAME(gtdf2)(sf_double a, sf_double b) { return SF_NAME(gedf2)(a, b); }
int SF_NAME(unorddf2)(sf_double a, sf_double b) { return cmp_bits_64(d2b(a), d2b(b), 2) == 2; }

sf_double SF_NAME(floatsidf)(int32_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)(int64_t)x) : from_mag(0, (uint64_t)x)); }
sf_double SF_NAME(floatunsidf)(uint32_t x) { return b2d(from_mag(0, x)); }
sf_double SF_NAME(floatdidf)(int64_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x)); }
sf_double SF_NAME(floatundidf)(uint64_t x) { return b2d(from_mag(0, x)); }
int32_t SF_NAME(fixdfsi)(sf_double a) { return (int32_t)to_signed(d2b(a), 32); }
uint32_t SF_NAME(fixunsdfsi)(sf_double a) { return (uint32_t)to_unsigned(d2b(a), 32); }
int64_t SF_NAME(fixdfdi)(sf_double a) { return to_signed(d2b(a), 64); }
uint64_t SF_NAME(fixunsdfdi)(sf_double a) { return to_unsigned(d2b(a), 64); }

#include "sf_core_end.inc"
