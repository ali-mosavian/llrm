/* Width helpers of the soft-float core. Each is the generic C body; a target
 * overrides one by defining its macro before this header (see README). */
#ifndef SF_HELPERS_H
#define SF_HELPERS_H

#include <stdint.h>

#ifndef SF_CLZ32
static inline int sf_clz32(uint32_t x)
{
    int n = 0;
    if (!(x & 0xFFFF0000u)) { n += 16; x <<= 16; }
    if (!(x & 0xFF000000u)) { n += 8; x <<= 8; }
    if (!(x & 0xF0000000u)) { n += 4; x <<= 4; }
    if (!(x & 0xC0000000u)) { n += 2; x <<= 2; }
    if (!(x & 0x80000000u)) { n += 1; }
    return n;
}
#else
#define sf_clz32 SF_CLZ32
#endif

static inline int sf_clz64(uint64_t x)
{
    uint32_t hi = (uint32_t)(x >> 32);
    return hi ? sf_clz32(hi) : 32 + sf_clz32((uint32_t)x);
}

/* x >> n, with every dropped bit folded into bit 0. */
static inline uint32_t sf_shr_sticky32(uint32_t x, int n)
{
    if (n <= 0) return x;
    if (n >= 32) return x != 0;
    return (x >> n) | ((x << (32 - n)) != 0);
}

#ifdef SF_SHIFT_STEPS
/* The same as constant steps (bytes, then single bits), for a target whose shift libcall loops one bit per iteration (6502). Only the addition uses it: where
 * the shift is rare (multiply, divide, pack) the steps cost more bytes than the cycles they save. */
static inline uint32_t sf_shr_sticky32_add(uint32_t x, int n)
{
    uint32_t lost = 0;
    if (n <= 0) return x;
    if (n >= 32) return x != 0;
    while (n >= 8) { lost |= x & 0xFFu; x >>= 8; n -= 8; }
    while (n > 0) { lost |= x & 1u; x >>= 1; n--; }
    return x | (lost != 0);
}
#else
#define sf_shr_sticky32_add sf_shr_sticky32
#endif

static inline uint64_t sf_shr_sticky64(uint64_t x, int n)
{
    if (n <= 0) return x;
    if (n >= 64) return x != 0;
    return (x >> n) | ((x << (64 - n)) != 0);
}

/* 32x32 -> 64 as hi:lo from 16x16 partial products: no 64-bit type needed. A target with a widening multiply defines SF_MUL32X32(a, b, &hi, &lo); per 400 calls a uint64 product cuts x86 m32 mul clocks 121.6k -> 92.7k (gcc) and x86 m16 mul 166k -> 128k. */
#if defined(SF_MUL32X32_6502) && !defined(SF_MUL32X32)
void __mul32x32(uint32_t a, uint32_t b, uint32_t *hi, uint32_t *lo); /* src/6502/mul32x32.s */
#define SF_MUL32X32 __mul32x32
#endif
#ifndef SF_MUL32X32
static inline void sf_mul32x32(uint32_t a, uint32_t b, uint32_t *hi, uint32_t *lo)
{
    uint32_t al = a & 0xFFFFu, ah = a >> 16, bl = b & 0xFFFFu, bh = b >> 16;
    uint32_t ll = al * bl, lh = al * bh, hl = ah * bl, hh = ah * bh;
    uint32_t mid = (ll >> 16) + (lh & 0xFFFFu) + (hl & 0xFFFFu);
    *lo = (ll & 0xFFFFu) | (mid << 16);
    *hi = hh + (lh >> 16) + (hl >> 16) + (mid >> 16);
}
#else
#define sf_mul32x32 SF_MUL32X32
#endif

/* 24x24 -> 48 as hi:lo by shift-add, one multiplier bit per step: no multiply instruction, a few bytes of code. */
static inline void sf_mul24x24_loop(uint32_t a, uint32_t b, uint32_t *hi, uint32_t *lo)
{
    uint32_t h = 0, l = b;
    for (int i = 0; i < 24; i++) {
        if (l & 1) h += a;
        l = (l >> 1) | (h << 31);
        h >>= 1;
    }
    *lo = (h << 24) | (l >> 8);
    *hi = h >> 8;
}

#endif
