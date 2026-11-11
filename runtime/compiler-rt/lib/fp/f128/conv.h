/* Shared by the integer to binary128 conversions (float.c, float_ti.c). The binary128 to integer ones are one file each: fix_si fix_usi fix_di fix_udi fix_ti fix_uti. */
#include "f128.h"

#ifndef SF_CONV128_H
#define SF_CONV128_H
static inline sf128 from_mag(sf128 sign, sf128 u)
{
    if (!u) return 0;
    int n = 127 - sf_clz128(u);
    sf128 m = n > 115 ? sf_shr_sticky128(u, n - 115) : u << (115 - n);
    return SF_PACK(sign, n + 16383, m);
}
#endif
