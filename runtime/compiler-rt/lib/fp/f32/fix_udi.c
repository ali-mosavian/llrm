/* __fixunssfdi, as fix_di.c. */
#include "fix_di.h"

static uint64_t to_u64_bits(sf32 a)
{
    int big;
    uint64_t m = magnitude(a, &big);
#ifdef SF_FIX_NAN_UNDEF
    if (a & SIGN) return 0;
#else
    if (((a << 1) > 0xFF000000u) || (a & SIGN)) return 0;
#endif
    return big ? UINT64_MAX : m;
}

uint64_t SF_NAME(fixunssfdi)(sf_float a) { return to_u64_bits(f2b(a)); }
