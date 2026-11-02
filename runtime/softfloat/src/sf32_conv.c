/* binary32 <-> 32-bit integer; no 64-bit type, so every target can build it. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

/* m has three guard bits and its implicit one at bit 26 (same rounding as sf32.c). */
sf32 sf32_pack(uint32_t sign, int e, uint32_t m)
{
    if (e >= 255) return sign | 0x7F800000u;
    if (e <= 0) { m = sf_shr_sticky32(m, 1 - e); e = 1; }
    uint32_t r = m & 7;
    m >>= 3;
    if (r > 4 || (r == 4 && (m & 1))) m++;
    return sign + ((uint32_t)(e - 1) << 23) + m;
}

static sf32 from_mag(uint32_t sign, uint32_t x)
{
    if (!x) return 0;
    int n = 31 - sf_clz32(x);
    uint32_t m = n > 26 ? sf_shr_sticky32(x, n - 26) : x << (26 - n);
    return sf32_pack(sign, n + 127, m);
}

sf32 sf32_from_u32(uint32_t x) { return from_mag(0, x); }
sf32 sf32_from_i32(int32_t x) { return x < 0 ? from_mag(SIGN, 0u - (uint32_t)x) : from_mag(0, (uint32_t)x); }

/* |a| truncated; *big is set from 2^32 up, where the result saturates. */
static uint32_t magnitude(sf32 a, int *big)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    *big = 0;
    if (e < 0) return 0;
    if (e >= 32) { *big = 1; return 0; }
    uint32_t m = (a & 0x7FFFFFu) | 0x800000u;
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}

int32_t sf32_to_i32(sf32 a)
{
    int big;
    uint32_t m = magnitude(a, &big);
    if ((a << 1) > 0xFF000000u) return 0;
    if (a & SIGN) return (big || m > 0x80000000u) ? INT32_MIN : (int32_t)(0u - m);
    return (big || m > 0x7FFFFFFFu) ? INT32_MAX : (int32_t)m;
}

uint32_t sf32_to_u32(sf32 a)
{
    int big;
    uint32_t m = magnitude(a, &big);
    if (((a << 1) > 0xFF000000u) || (a & SIGN)) return 0;
    return big ? UINT32_MAX : m;
}
