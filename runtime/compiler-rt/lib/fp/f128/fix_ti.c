/* __fixtfti: truncate toward zero; out of range saturates, NaN is 0 unless SF_FIX_NAN_UNDEF. */
#include "f128.h"

__int128 SF_NAME(fixtfti)(sf_quad d)
{
    sf128 a = q2b(d);
    int e = (int)((a >> 112) & 0x7FFF) - 16383;
    if (e < 0) return 0;
    if (e >= 127) {
#ifndef SF_FIX_NAN_UNDEF
        if (is_nan_128(a)) return 0;
#endif
        return (a & SF_SIGN) ? ((__int128)1 << 127) : (__int128)(((sf128)1 << 127) - 1);
    }
    sf128 m = (a & (SF_HID - 1)) | SF_HID;
    m = e >= 112 ? m << (e - 112) : m >> (112 - e);
    return (a & SF_SIGN) ? (__int128)((sf128)0 - m) : (__int128)m;
}

#include "../core_end.inc"
