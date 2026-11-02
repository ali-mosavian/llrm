/* 32-bit multiply and divide for CPUs without the instruction. */
#include "sf_int.h"
#include "sf_helpers.h"

uint32_t SFB_NAME(mulsi3)(uint32_t a, uint32_t b)
{
    uint32_t al = a & 0xFFFFu, bl = b & 0xFFFFu;
    return al * bl + (((a >> 16) * bl + al * (b >> 16)) << 16);
}

/* Shift-subtract on operands aligned by clz; 16-bit operands take the short path. */
static uint32_t udivmod(uint32_t n, uint32_t d, uint32_t *r)
{
    uint32_t q = 0;
    if (!d) { *r = n; return ~0u; }
    if (d > n) { *r = n; return 0; }
    if (!((n | d) >> 16)) { *r = (uint16_t)n % (uint16_t)d; return (uint16_t)n / (uint16_t)d; }
    int s = sf_clz32(d) - sf_clz32(n);
    d <<= s;
    for (int i = 0; i <= s; i++) {
        q <<= 1;
        if (n >= d) { n -= d; q |= 1; }
        d >>= 1;
    }
    *r = n;
    return q;
}

uint32_t SFB_NAME(udivsi3)(uint32_t n, uint32_t d) { uint32_t r; return udivmod(n, d, &r); }
uint32_t SFB_NAME(umodsi3)(uint32_t n, uint32_t d) { uint32_t r; udivmod(n, d, &r); return r; }

int32_t SFB_NAME(divsi3)(int32_t n, int32_t d)
{
    uint32_t r, q = udivmod(n < 0 ? 0u - (uint32_t)n : (uint32_t)n, d < 0 ? 0u - (uint32_t)d : (uint32_t)d, &r);
    return (int32_t)((n < 0) != (d < 0) && d ? 0u - q : q);
}

int32_t SFB_NAME(modsi3)(int32_t n, int32_t d)
{
    uint32_t r;
    udivmod(n < 0 ? 0u - (uint32_t)n : (uint32_t)n, d < 0 ? 0u - (uint32_t)d : (uint32_t)d, &r);
    return (int32_t)(n < 0 ? 0u - r : r);
}
