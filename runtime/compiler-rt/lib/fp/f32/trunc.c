/* __truncdfsf2: binary64 to binary32. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

static sf32 trunc_bits(sf64 a)
{
    uint32_t sign = (uint32_t)(a >> 32) & SIGN;
    int e = (int)((a >> 52) & 0x7FF);
    uint64_t m = a & 0xFFFFFFFFFFFFFull;
    if (e == 0x7FF) return m ? (sign | 0x7FC00000u | (uint32_t)(m >> 29)) : sign | 0x7F800000u;
    if (!e) return sign;
    m |= 1ull << 52;
    return sf32_pack(sign, e - 896, (uint32_t)sf_shr_sticky64(m, 26));
}

sf_float SF_NAME(truncdfsf2)(sf_double a) { return b2f(trunc_bits(d2b(a))); }
