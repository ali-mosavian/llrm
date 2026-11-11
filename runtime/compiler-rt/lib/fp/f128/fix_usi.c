/* __fixunstfsi: truncate toward zero; out of range saturates, NaN is 0 unless SF_FIX_NAN_UNDEF. */
#include "f128.h"

uint32_t SF_NAME(fixunstfsi)(sf_quad d)
{
    sf128 a = q2b(d);
    int e = (int)((a >> 112) & 0x7FFF) - 16383;
    if (e < 0 || (a & SF_SIGN)) return 0;
    if (e >= 32) {
#ifndef SF_FIX_NAN_UNDEF
        if (is_nan_128(a)) return 0;
#endif
        return UINT32_MAX;
    }
    sf128 m = (a & (SF_HID - 1)) | SF_HID;
    m = e >= 112 ? m << (e - 112) : m >> (112 - e);
    return (uint32_t)m;
}

#include "../core_end.inc"
