/* __fixsfdi; needs a 64-bit type. */
#include "fix_di.h"

static int64_t to_i64_bits(sf32 a)
{
    int big;
    uint64_t m = magnitude(a, &big);
#ifndef SF_FIX_NAN_UNDEF
    if ((a << 1) > 0xFF000000u) return 0;
#endif
    if (a & SIGN) return (big || m > 0x8000000000000000ull) ? INT64_MIN : (int64_t)(0ull - m);
    return (big || m > 0x7FFFFFFFFFFFFFFFull) ? INT64_MAX : (int64_t)m;
}

int64_t SF_NAME(fixsfdi)(sf_float a) { return to_i64_bits(f2b(a)); }
