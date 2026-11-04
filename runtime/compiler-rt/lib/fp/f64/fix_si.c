/* __fixdfsi on 32-bit arithmetic: the top 32 bits of the 53-bit significand hold every integer bit of a result below 2^31. */
#include "f64.h"

int32_t SF_NAME(fixdfsi)(sf_double d)
{
    sf64 a = d2b(d);
    uint32_t hi = (uint32_t)(a >> 32), lo = (uint32_t)a;
    int e = (int)((hi >> 20) & 0x7FF) - 1023;
    if (e < 0) return 0;
    if (e >= 31) {
#ifndef SF_FIX_NAN_UNDEF
        if ((hi << 1) > 0xFFE00000u || ((hi << 1) == 0xFFE00000u && lo)) return 0;
#endif
        return (hi >> 31) ? INT32_MIN : INT32_MAX;
    }
    uint32_t m = (((hi & 0xFFFFFu) | 0x100000u) << 11 | lo >> 21) >> (31 - e);
    return (hi >> 31) ? (int32_t)(0u - m) : (int32_t)m;
}

#include "../core_end.inc"
