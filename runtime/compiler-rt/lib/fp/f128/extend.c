/* __extendsftf2, __extenddftf2: binary32 and binary64 to binary128, exact; a NaN keeps its payload and becomes quiet. */
#include "f128.h"

static sf128 ext_bits(sf128 sign, int e, uint64_t m, int mb, int expmax, int bias, int clz_base)
{
    sf128 mm = m;
    if (e == expmax) return sign | ((sf128)0x7FFF << 112) | (mm << (112 - mb)) | (m ? (sf128)1 << 111 : 0);
    if (!e) {
        if (!m) return sign;
        int s = sf_clz64(m) - clz_base;
        mm = (mm << s) & (((sf128)1 << mb) - 1);
        e = 1 - s;
    }
    return sign | ((sf128)(e + 16383 - bias) << 112) | (mm << (112 - mb));
}

sf_quad SF_NAME(extendsftf2)(sf_float a)
{
    sf32 x = f2b(a);
    return b2q(ext_bits((sf128)(x >> 31) << 127, (int)((x >> 23) & 0xFF), x & 0x7FFFFFu, 23, 255, 127, 63 - 23));
}

sf_quad SF_NAME(extenddftf2)(sf_double a)
{
    sf64 x = d2b(a);
    return b2q(ext_bits((sf128)(x >> 63) << 127, (int)((x >> 52) & 0x7FF), x & 0xFFFFFFFFFFFFFull, 52, 2047, 1023, 63 - 52));
}

#include "../core_end.inc"
