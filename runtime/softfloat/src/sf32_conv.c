/* binary32 <-> integer and binary32 <-> binary64. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

static sf32 pack(uint32_t sign, int e, uint32_t m)
{
    /* Same rounding as sf32.c: m has three guard bits, implicit one at bit 26. */
    if (e >= 255) return sign | 0x7F800000u;
    if (e <= 0) { m = sf_shr_sticky32(m, 1 - e); e = 1; }
    uint32_t r = m & 7;
    m >>= 3;
    if (r > 4 || (r == 4 && (m & 1))) m++;
    return sign + ((uint32_t)(e - 1) << 23) + m;
}

static sf32 from_u64(uint32_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint32_t m = n > 26 ? (uint32_t)sf_shr_sticky64(u, n - 26) : (uint32_t)u << (26 - n);
    return pack(sign, n + 127, m);
}

sf32 sf32_from_u32(uint32_t x)
{
    if (!x) return 0;
    int n = 31 - sf_clz32(x);
    uint32_t m = n > 26 ? sf_shr_sticky32(x, n - 26) : x << (26 - n);
    return pack(0, n + 127, m);
}

sf32 sf32_from_i32(int32_t x)
{
    if (x >= 0) return sf32_from_u32((uint32_t)x);
    uint32_t u = 0u - (uint32_t)x, m;
    int n = 31 - sf_clz32(u);
    m = n > 26 ? sf_shr_sticky32(u, n - 26) : u << (26 - n);
    return pack(SIGN, n + 127, m);
}

sf32 sf32_from_u64(uint64_t x) { return from_u64(0, x); }
sf32 sf32_from_i64(int64_t x) { return x < 0 ? from_u64(SIGN, 0ull - (uint64_t)x) : from_u64(0, (uint64_t)x); }

/* Magnitude of |a| as a 64-bit integer, truncated; *big is set at 2^64 and above. */
static uint64_t magnitude(sf32 a, int *big)
{
    int e = (a >> 23) & 0xFF;
    *big = 0;
    if (e < 127) return 0;
    uint64_t m = (a & 0x7FFFFFu) | 0x800000u;
    e -= 127;
    if (e >= 64) { *big = 1; return 0; }
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}

static int is_nan(sf32 a) { return (a << 1) > 0xFF000000u; }

int64_t sf32_to_i64(sf32 a)
{
    int big;
    uint64_t m = magnitude(a, &big);
    if (is_nan(a)) return 0;
    if (a & SIGN) return (big || m > 0x8000000000000000ull) ? INT64_MIN : (int64_t)(0ull - m);
    return (big || m > 0x7FFFFFFFFFFFFFFFull) ? INT64_MAX : (int64_t)m;
}

uint64_t sf32_to_u64(sf32 a)
{
    int big;
    uint64_t m = magnitude(a, &big);
    if (is_nan(a) || (a & SIGN)) return 0;
    return big ? UINT64_MAX : m;
}

int32_t sf32_to_i32(sf32 a)
{
    int64_t v = sf32_to_i64(a);
    return v > INT32_MAX ? INT32_MAX : v < INT32_MIN ? INT32_MIN : (int32_t)v;
}

uint32_t sf32_to_u32(sf32 a)
{
    uint64_t v = sf32_to_u64(a);
    return v > UINT32_MAX ? UINT32_MAX : (uint32_t)v;
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
    return pack(sign, e - 896, (uint32_t)sf_shr_sticky64(m, 26));
}
