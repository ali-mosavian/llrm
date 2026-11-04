/* __fixdfdi: the 53-bit significand left-justified to bit 63, one right shift by 63 - e. */
#include "f64.h"

int64_t SF_NAME(fixdfdi)(sf_double d)
{
    sf64 a = d2b(d);
    int e = (int)((a >> 52) & 0x7FF) - 1023;
    if (e < 0) return 0;
    if (e >= 63) {
#ifndef SF_FIX_NAN_UNDEF
        if (is_nan_64(a)) return 0;
#endif
        return (a & SF_SIGN) ? INT64_MIN : INT64_MAX;
    }
    uint64_t m = (((a & (SF_HID - 1)) | SF_HID) << 11) >> (63 - e);
    uint64_t s = (uint64_t)((int64_t)a >> 63); /* all ones for a negative a */
    return (int64_t)((m ^ s) - s);
}

#include "../core_end.inc"
