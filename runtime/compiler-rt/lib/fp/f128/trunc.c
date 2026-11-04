/* __trunctfsf2, __trunctfdf2: binary128 to binary32 and binary64, rounded to nearest even. */
#include "f128.h"

uint64_t sf64_pack(uint64_t sign, int e, uint64_t m); /* f64/pack.c */

sf_float SF_NAME(trunctfsf2)(sf_quad a)
{
    sf128 x = q2b(a);
    uint32_t sign = (uint32_t)(x >> 96) & 0x80000000u;
    int e = (int)((x >> 112) & 0x7FFF);
    sf128 m = x & (SF_HID - 1);
    if (e == 0x7FFF) return b2f(m ? (sign | 0x7FC00000u | (uint32_t)(m >> 89)) : sign | 0x7F800000u);
    if (!e) return b2f(sign);
    m |= SF_HID;
    return b2f(sf32_pack(sign, e - 16256, (uint32_t)sf_shr_sticky128(m, 86)));
}

sf_double SF_NAME(trunctfdf2)(sf_quad a)
{
    sf128 x = q2b(a);
    uint64_t sign = (uint64_t)(x >> 64) & 0x8000000000000000ull;
    int e = (int)((x >> 112) & 0x7FFF);
    sf128 m = x & (SF_HID - 1);
    if (e == 0x7FFF) return b2d(m ? (sign | 0x7FF8000000000000ull | (uint64_t)(m >> 60)) : sign | 0x7FF0000000000000ull);
    if (!e) return b2d(sign);
    m |= SF_HID;
    return b2d(sf64_pack(sign, e - 15360, (uint64_t)sf_shr_sticky128(m, 57)));
}

#include "../core_end.inc"
