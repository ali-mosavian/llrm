/* __fixunstfdi: truncate toward zero; out of range saturates, NaN is 0 unless SF_FIX_NAN_UNDEF. */
#include "f128.h"

uint64_t SF_NAME(fixunstfdi)(sf_quad d)
{
    sf128 a = q2b(d);
    int e = (int)((a >> 112) & 0x7FFF) - 16383;
    if (e < 0 || (a & SF_SIGN)) return 0;
    if (e >= 64) {
#ifndef SF_FIX_NAN_UNDEF
        if (is_nan_128(a)) return 0;
#endif
        return UINT64_MAX;
    }
    sf128 m = (a & (SF_HID - 1)) | SF_HID;
    m = e >= 112 ? m << (e - 112) : m >> (112 - e);
    return (uint64_t)m;
}

#include "../core_end.inc"
