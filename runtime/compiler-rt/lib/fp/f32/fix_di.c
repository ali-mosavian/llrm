/* __fixsfdi, __fixunssfdi; needs a 64-bit type. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

/* |a| truncated; *big is set from 2^64 up. */
static uint64_t magnitude(sf32 a, int *big)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    *big = 0;
    if (e < 0) return 0;
    if (e >= 64) { *big = 1; return 0; }
    uint64_t m = (a & 0x7FFFFFu) | 0x800000u;
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}

static int64_t to_i64_bits(sf32 a)
{
    int big;
    uint64_t m = magnitude(a, &big);
    if ((a << 1) > 0xFF000000u) return 0;
    if (a & SIGN) return (big || m > 0x8000000000000000ull) ? INT64_MIN : (int64_t)(0ull - m);
    return (big || m > 0x7FFFFFFFFFFFFFFFull) ? INT64_MAX : (int64_t)m;
}

static uint64_t to_u64_bits(sf32 a)
{
    int big;
    uint64_t m = magnitude(a, &big);
    if (((a << 1) > 0xFF000000u) || (a & SIGN)) return 0;
    return big ? UINT64_MAX : m;
}

int64_t SF_NAME(fixsfdi)(sf_float a) { return to_i64_bits(f2b(a)); }
uint64_t SF_NAME(fixunssfdi)(sf_float a) { return to_u64_bits(f2b(a)); }
