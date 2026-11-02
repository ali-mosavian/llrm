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

static inline uint64_t sf_shr_sticky64(uint64_t x, int n)
{
    if (n <= 0) return x;
    if (n >= 64) return x != 0;
    return (x >> n) | ((x << (64 - n)) != 0);
}

#ifndef SF_MUL32X32
static inline uint64_t sf_mul32x32(uint32_t a, uint32_t b) { return (uint64_t)a * b; }
#else
#define sf_mul32x32 SF_MUL32X32
#endif

#endif
