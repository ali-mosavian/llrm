/* IEEE-754 binary32, round to nearest even, denormals, inf and NaN.
 * Working form: 27-bit mantissa (implicit one at bit 26, three guard bits). */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u
#define INF 0x7F800000u

static inline int is_nan(sf32 a) { return (a << 1) > (INF << 1); }

/* e is a biased exponent that may be <= 0 (denormal) or >= 255 (overflow). */
static sf32 round_pack(uint32_t sign, int e, uint32_t m)
{
    if (e >= 255) return sign | INF;
    if (e <= 0) {
        m = sf_shr_sticky32(m, 1 - e);
        e = 1;
    }
    uint32_t r = m & 7;
    m >>= 3;
    if (r > 4 || (r == 4 && (m & 1))) m++;
    return sign + ((uint32_t)(e - 1) << 23) + m;
}

/* Mantissa with three guard bits and exponent; denormals get e = 1, no implicit bit. */
static inline uint32_t unpack(sf32 a, int *e)
{
    uint32_t m = a & 0x7FFFFFu;
    int x = (a >> 23) & 0xFF;
    if (x) m |= 0x800000u; else x = 1;
    *e = x;
    return m;
}

sf32 sf32_add(sf32 a, sf32 b)
{
    if (((a << 1) >= (INF << 1)) | ((b << 1) >= (INF << 1))) {
        if (is_nan(a)) return a | 0x400000u;
        if (is_nan(b)) return b | 0x400000u;
        if ((a << 1) == (INF << 1) && (b << 1) == (INF << 1) && ((a ^ b) & SIGN)) return SF32_QNAN;
        return (a << 1) == (INF << 1) ? a : b;
    }
    if ((a << 1) == 0 || (b << 1) == 0) {
        if ((a << 1) == 0 && (b << 1) == 0) return a & b;
        return (a << 1) == 0 ? b : a;
    }
    if ((a & ~SIGN) < (b & ~SIGN)) { sf32 t = a; a = b; b = t; }
    int ea, eb;
    uint32_t ma = unpack(a, &ea) << 3, mb = unpack(b, &eb) << 3;
    mb = sf_shr_sticky32(mb, ea - eb);
    uint32_t sign = a & SIGN, m;
    if ((a ^ b) & SIGN) {
        m = ma - mb;
        if (!m) return 0;
        int s = sf_clz32(m) - 5;
        if (s > ea - 1) s = ea - 1;
        m <<= s;
        ea -= s;
    } else {
        m = ma + mb;
        if (m & (UINT32_C(1) << 27)) { m = sf_shr_sticky32(m, 1); ea++; }
    }
    return round_pack(sign, ea, m);
}

sf32 sf32_sub(sf32 a, sf32 b) { return sf32_add(a, b ^ SIGN); }

sf32 sf32_mul(sf32 a, sf32 b)
{
    uint32_t sign = (a ^ b) & SIGN;
    if (((a << 1) >= (INF << 1)) | ((b << 1) >= (INF << 1))) {
        if (is_nan(a)) return a | 0x400000u;
        if (is_nan(b)) return b | 0x400000u;
        if ((a << 1) == 0 || (b << 1) == 0) return SF32_QNAN;
        return sign | INF;
    }
    if ((a << 1) == 0 || (b << 1) == 0) return sign;
    int ea, eb;
    uint32_t ma = unpack(a, &ea), mb = unpack(b, &eb);
    if (!(ma & 0x800000u)) { int s = sf_clz32(ma) - 8; ma <<= s; ea -= s; }
    if (!(mb & 0x800000u)) { int s = sf_clz32(mb) - 8; mb <<= s; eb -= s; }
    uint32_t hi, lo;
    sf_mul32x32(ma, mb, &hi, &lo);
    /* The product has its top bit at 47 or 46; keep 27 bits and fold the rest. */
    int e = ea + eb - 127, sh = 20;
    if (hi >> 15) { sh = 21; e++; }
    uint32_t m = (hi << (32 - sh)) | (lo >> sh) | ((lo << (32 - sh)) != 0);
    return round_pack(sign, e, m);
}

sf32 sf32_div(sf32 a, sf32 b)
{
    uint32_t sign = (a ^ b) & SIGN;
    if (((a << 1) >= (INF << 1)) | ((b << 1) >= (INF << 1))) {
        if (is_nan(a)) return a | 0x400000u;
        if (is_nan(b)) return b | 0x400000u;
        if ((a << 1) == (INF << 1)) return (b << 1) == (INF << 1) ? SF32_QNAN : sign | INF;
        return sign;
    }
    if ((b << 1) == 0) return (a << 1) == 0 ? SF32_QNAN : sign | INF;
    if ((a << 1) == 0) return sign;
    int ea, eb;
    uint32_t ma = unpack(a, &ea), mb = unpack(b, &eb);
    if (!(ma & 0x800000u)) { int s = sf_clz32(ma) - 8; ma <<= s; ea -= s; }
    if (!(mb & 0x800000u)) { int s = sf_clz32(mb) - 8; mb <<= s; eb -= s; }
    /* q = floor(ma * 2^27 / mb), one quotient bit per step. */
    uint32_t q = 0, r = ma;
    for (int i = 0; i < 28; i++) {
        q <<= 1;
        if (r >= mb) { r -= mb; q |= 1; }
        r <<= 1;
    }
    int e = ea - eb + 126;
    if (q >> 27) { q = (q >> 1) | (q & 1); e++; }
    return round_pack(sign, e, q | (r != 0));
}

int sf32_cmp(sf32 a, sf32 b)
{
    if (is_nan(a) || is_nan(b)) return 2;
    if (!((a | b) << 1)) return 0;
    int32_t x = (int32_t)a, y = (int32_t)b;
    if (x < 0) x = (int32_t)SIGN - x;
    if (y < 0) y = (int32_t)SIGN - y;
    return x < y ? -1 : x > y;
}
