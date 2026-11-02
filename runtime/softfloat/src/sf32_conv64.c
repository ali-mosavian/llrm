/* binary32 <-> 64-bit integer and binary64; needs a 64-bit type. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

sf32 sf32_pack(uint32_t sign, int e, uint32_t m);

static sf32 from_mag(uint32_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint32_t m = n > 26 ? (uint32_t)sf_shr_sticky64(u, n - 26) : (uint32_t)u << (26 - n);
    return sf32_pack(sign, n + 127, m);
}

sf32 sf32_from_u64(uint64_t x) { return from_mag(0, x); }
sf32 sf32_from_i64(int64_t x) { return x < 0 ? from_mag(SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x); }

/* |a| truncated; *big is set from 2^64 up. */
static uint64_t magnitude(sf32 a, int *big)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    *big = 0;
    if (e < 0) return 0;
    if (e >= 64) { *big = 1; return 0; }
    uint64_t m = (a & 0x7FFFFFu) | 0x800000u;
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}

int64_t sf32_to_i64(sf32 a)
{
    int big;
    uint64_t m = magnitude(a, &big);
    if ((a << 1) > 0xFF000000u) return 0;
    if (a & SIGN) return (big || m > 0x8000000000000000ull) ? INT64_MIN : (int64_t)(0ull - m);
    return (big || m > 0x7FFFFFFFFFFFFFFFull) ? INT64_MAX : (int64_t)m;
}

uint64_t sf32_to_u64(sf32 a)
{
    int big;
    uint64_t m = magnitude(a, &big);
    if (((a << 1) > 0xFF000000u) || (a & SIGN)) return 0;
    return big ? UINT64_MAX : m;
}

sf64 sf32_to_sf64(sf32 a)
{
    uint64_t sign = (uint64_t)(a & SIGN) << 32;
    int e = (a >> 23) & 0xFF;
    uint64_t m = a & 0x7FFFFFu;
    if (e == 255) return sign | 0x7FF0000000000000ull | (m << 29) | (m ? 0x0008000000000000ull : 0);
    if (!e) {
        if (!m) return sign;
        int s = sf_clz32((uint32_t)m) - 8;
        m = (m << s) & 0x7FFFFFu;
        e = 1 - s;
    }
    return sign | ((uint64_t)(e + 896) << 52) | (m << 29);
}

sf32 sf64_to_sf32(sf64 a)
{
    uint32_t sign = (uint32_t)(a >> 32) & SIGN;
    int e = (int)((a >> 52) & 0x7FF);
    uint64_t m = a & 0xFFFFFFFFFFFFFull;
    if (e == 0x7FF) return m ? (sign | 0x7FC00000u | (uint32_t)(m >> 29)) : sign | 0x7F800000u;
    if (!e) return sign;
    m |= 1ull << 52;
    return sf32_pack(sign, e - 896, (uint32_t)sf_shr_sticky64(m, 26));
}
