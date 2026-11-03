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
#endif
