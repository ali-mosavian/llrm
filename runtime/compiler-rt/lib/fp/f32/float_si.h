/* Shared by float_si.c and float_usi.c; each file keeps its own static copy (one operation per object). */
#ifndef SF_FLOAT_SI_H
#define SF_FLOAT_SI_H
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

static sf32 from_mag(uint32_t sign, uint32_t x)
{
    if (!x) return 0;
    int n = 31 - sf_clz32(x);
    uint32_t m = n > 26 ? sf_shr_sticky32(x, n - 26) : x << (26 - n);
    return pack_round(sign, n + 127, m); /* n <= 31: e is 127..158 */
}

#endif
