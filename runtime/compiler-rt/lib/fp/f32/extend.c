/* __extendsfdf2: binary32 to binary64. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

static sf64 ext_bits(sf32 a)
{
    uint64_t sign = (uint64_t)(a & SIGN) << 32;
    int e = (a >> 23) & 0xFF;
    uint64_t m = a & 0x7FFFFFu;
    if (e == 255) return sign | 0x7FF0000000000000ull | (m << 29) | (m ? 0x0008000000000000ull : 0);
    if (!e) {
        if (!m) return sign;
        int s = sf_clz32((uint32_t)m) - 8;
        m = (m << s) & 0x7FFFFFu;
        e = 1 - s;
    }
    return sign | ((uint64_t)(e + 896) << 52) | (m << 29);
}

sf_double SF_NAME(extendsfdf2)(sf_float a) { return b2d(ext_bits(f2b(a))); }
