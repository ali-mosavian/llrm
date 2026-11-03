/* __fixunssfsi; its own file so a program that needs only one of the two links only that one. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

static uint32_t to_u32_bits(sf32 a)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    if (e < 0 || (a & SIGN)) return 0;
    if (e >= 32) return (a << 1) > 0xFF000000u ? 0 : UINT32_MAX;
    uint32_t m = (a & 0x7FFFFFu) | 0x800000u;
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}

uint32_t SF_NAME(fixunssfsi)(sf_float a) { return to_u32_bits(f2b(a)); }
