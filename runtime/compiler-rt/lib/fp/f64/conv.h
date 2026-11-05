/* Shared by the integer to binary64 conversions (float.c). The binary64 to integer ones are one file each: fix_si fix_usi fix_di fix_udi. */
#include "f64.h"

#ifndef SF_CONV64_H
#define SF_CONV64_H
/* A 32-bit magnitude always fits the 53-bit significand: no rounding, and every shift stays in 32-bit halves. */
#ifdef SF_CONV_NOINLINE /* wasm -O2 inlines both helpers: f64 float.c 287 -> 655 B */
#define SF_CONV_FN __attribute__((noinline)) static
#else
#define SF_CONV_FN static
#endif
#ifndef SF_NO_EXACT32
/* Not `inline`: the callers must share one copy (m68k -Os: two inlined copies grew float.c by 324 B). */
/* A 32-bit magnitude always fits the 53-bit significand: no rounding, and every shift stays in 32-bit halves. */
SF_CONV_FN sf64 from_mag32(uint64_t sign, uint32_t x)
{
    if (!x) return 0;
    int n = 31 - sf_clz32(x);
    uint32_t y = x << (31 - n); /* top bit set */
    return sign + (((uint64_t)(((uint32_t)n + 1022u) << 20) + (y >> 11)) << 32) + ((uint64_t)(y << 21));
}
SF_CONV_FN sf64 from_mag(uint64_t sign, uint64_t u)
{
    uint32_t hi = (uint32_t)(u >> 32);
    if (!hi) return from_mag32(sign, (uint32_t)u);
    int n = 63 - sf_clz32(hi);
    uint64_t m = n > 55 ? sf_shr_sticky64(u, n - 55) : u << (55 - n);
    return SF_PACK(sign, n + 1023, m);
}
#else /* SF_NO_EXACT32: one 64-bit path, no second normaliser (x86 m32 -Os: the second path costs +25 B) */
static inline sf64 from_mag(uint64_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint64_t m = n > 55 ? sf_shr_sticky64(u, n - 55) : u << (55 - n);
    return SF_PACK(sign, n + 1023, m);
}
#endif
#endif
