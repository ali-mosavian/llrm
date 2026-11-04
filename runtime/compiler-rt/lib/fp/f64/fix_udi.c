/* __fixunsdfdi, as fix_di.c. */
#include "f64.h"

uint64_t SF_NAME(fixunsdfdi)(sf_double d)
{
    sf64 a = d2b(d);
    int e = (int)((a >> 52) & 0x7FF) - 1023;
    if (e < 0 || (a & SF_SIGN)) return 0;
    if (e >= 64) {
#ifndef SF_FIX_NAN_UNDEF
        if (is_nan_64(a)) return 0;
#endif
        return UINT64_MAX;
    }
    return (((a & (SF_HID - 1)) | SF_HID) << 11) >> (63 - e);
}

#include "../core_end.inc"
