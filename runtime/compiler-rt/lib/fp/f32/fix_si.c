/* __fixsfsi, __fixunssfsi; no 64-bit type, so every target can build it. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

/* |a| truncated; *big is set from 2^32 up, where the result saturates. */
static int32_t to_i32_bits(sf32 a)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    if (e < 0) return 0;
    if (e >= 31) { /* from 2^31 up: saturates; -2^31 is the saturated value itself */
        if ((a << 1) > 0xFF000000u) return 0;
        return (a & SIGN) ? INT32_MIN : INT32_MAX;
    }
    uint32_t m = (a & 0x7FFFFFu) | 0x800000u;
    m = e >= 23 ? m << (e - 23) : m >> (23 - e);
    return (a & SIGN) ? (int32_t)(0u - m) : (int32_t)m;
}

static uint32_t to_u32_bits(sf32 a)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    if (e < 0 || (a & SIGN)) return 0;
    if (e >= 32) return (a << 1) > 0xFF000000u ? 0 : UINT32_MAX;
    uint32_t m = (a & 0x7FFFFFu) | 0x800000u;
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}

int32_t SF_NAME(fixsfsi)(sf_float a) { return to_i32_bits(f2b(a)); }
uint32_t SF_NAME(fixunssfsi)(sf_float a) { return to_u32_bits(f2b(a)); }
