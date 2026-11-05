/* Shared by the integer to binary64 conversions (float.c). The binary64 to integer ones are one file each: fix_si fix_usi fix_di fix_udi. */
#include "f64.h"

#ifndef SF_CONV64_H
#define SF_CONV64_H
static inline sf64 from_mag(uint64_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint64_t m = n > 55 ? sf_shr_sticky64(u, n - 55) : u << (55 - n);
    return SF_PACK(sign, n + 1023, m);
}
/* A 32-bit magnitude always fits the 53-bit significand: no rounding, and every shift stays in 32-bit halves. */
static inline sf64 from_mag32(uint64_t sign, uint32_t x)
{
    if (!x) return 0;
    int n = 31 - sf_clz32(x);
    uint32_t y = x << (31 - n); /* top bit set */
    return sign + (((uint64_t)(((uint32_t)n + 1022u) << 20) + (y >> 11)) << 32) + ((uint64_t)(y << 21));
}
#endif
