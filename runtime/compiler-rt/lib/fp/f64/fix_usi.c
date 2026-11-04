/* __fixunsdfsi on 32-bit arithmetic, as fix_si.c. */
#include "f64.h"

uint32_t SF_NAME(fixunsdfsi)(sf_double d)
{
    sf64 a = d2b(d);
    uint32_t hi = (uint32_t)(a >> 32), lo = (uint32_t)a;
    int e = (int)((hi >> 20) & 0x7FF) - 1023;
    if (e < 0 || (hi >> 31)) return 0;
    if (e >= 32) {
#ifndef SF_FIX_NAN_UNDEF
        if ((hi << 1) > 0xFFE00000u || ((hi << 1) == 0xFFE00000u && lo)) return 0;
#endif
        return UINT32_MAX;
    }
    return (((hi & 0xFFFFFu) | 0x100000u) << 11 | lo >> 21) >> (31 - e);
}

#include "../core_end.inc"
