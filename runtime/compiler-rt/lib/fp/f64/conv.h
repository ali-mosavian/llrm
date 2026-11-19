/* Shared by the integer to binary64 conversions (float.c). The binary64 to integer ones are one file each: fix_si fix_usi fix_di fix_udi. */
#include "f64.h"

#ifndef SF_CONV64_H
#define SF_CONV64_H
/* A 32-bit magnitude always fits the 53-bit significand: no rounding, and every shift stays in 32-bit halves. */
#if defined(SF_CONV_NOINLINE) || (defined(SF_CONV_DIRECT32) && !defined(SF_CLZ64)) /* wasm -O2 inlines both helpers: f64 float.c 287 -> 655 B */
#define SF_CONV_FN __attribute__((noinline)) static
#else
#define SF_CONV_FN static
#endif
/* n = index of the top set bit of u (u != 0); round to nearest even at bit 52. Needs no pack: n <= 63 means at most 11 bits are dropped, and a carry out of the
 * significand lands in the exponent field by the addition (the implicit bit is added with m). */
#ifdef SF_CLZ64
static inline sf64 round_mag(uint64_t sign, uint64_t u, int n)
{
    uint64_t m;
    if (n <= 52) m = u << (52 - n);
    else {
        int k = n - 52;
        uint64_t r = u & ((1ull << k) - 1), h = 1ull << (k - 1);
        m = u >> k;
        m += (r > h) | ((r == h) & (m & 1));
    }
    return sign + ((uint64_t)(n + 1022) << 52) + m;
}
#endif
#ifndef SF_NO_EXACT32
/* Not `inline`: the callers must share one copy (m68k -Os: two inlined copies grew float.c by 324 B). */
/* A 32-bit magnitude always fits the 53-bit significand: no rounding, and every shift stays in 32-bit halves. */
SF_CONV_FN sf64 from_mag32(uint64_t sign, uint32_t x)
{
    if (!x) return 0;
#if defined(SF_CLZ64) || defined(SF_CONV_WIDE32) /* 64-bit registers: one shift puts the top bit at 52 (compiler-rt's form); the implicit bit adds the last exponent step */
    int c = sf_clz32(x);
    return sign + (((uint64_t)x << (c + 21)) + ((uint64_t)(1053 - c) << 52));
#else
    int n = 31 - sf_clz32(x);
    uint32_t y = x << (31 - n); /* top bit set */
    return sign + (((uint64_t)(((uint32_t)n + 1022u) << 20) + (y >> 11)) << 32) + ((uint64_t)(y << 21));
#endif
}
SF_CONV_FN sf64 from_mag(uint64_t sign, uint64_t u)
{
#ifdef SF_CLZ64 /* native 64-bit clz: one normaliser and no pack call (a program using only conversions links 52-168 B, not 300) */
    if (!u) return 0;
    return round_mag(sign, u, 63 - sf_clz64(u));
#else /* pack is shared with the arithmetic: inlining the rounding grew float.c (m68k +126 B, x86 m64 +66) */
    uint32_t hi = (uint32_t)(u >> 32);
    if (!hi) return from_mag32(sign, (uint32_t)u);
    int n = 63 - sf_clz32(hi);
    uint64_t m = n > 55 ? sf_shr_sticky64(u, n - 55) : u << (55 - n);
    return SF_PACK(sign, n + 1023, m);
#endif
}
#else /* SF_NO_EXACT32: one 64-bit path, no second normaliser (m68k: the second path costs +12..20 B) */
static inline sf64 from_mag(uint64_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint64_t m = n > 55 ? sf_shr_sticky64(u, n - 55) : u << (55 - n);
    return SF_PACK(sign, n + 1023, m);
}
#endif
#endif
