/* __fixsfsi; no 64-bit type, so every target can build it. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

/* |a| truncated; *big is set from 2^32 up, where the result saturates. */
static int32_t to_i32_bits(sf32 a)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    if (e < 0) return 0;
    if (e >= 31) { /* from 2^31 up: saturates; -2^31 is the saturated value itself */
#ifndef SF_FIX_NAN_UNDEF
        if ((a << 1) > 0xFF000000u) return 0;
#endif
        return (a & SIGN) ? INT32_MIN : INT32_MAX;
    }
#ifdef SF_SHIFT_LOOP
    /* Shift cost grows with the count (no barrel shifter): the small count of the two-direction form wins. */
    uint32_t m = (a & 0x7FFFFFu) | 0x800000u;
    m = e >= 23 ? m << (e - 23) : m >> (23 - e);
#else
    /* The significand with its hidden bit at bit 31, shifted down once: one
     * shift in one direction, so no register is spilled across it. */
    uint32_t m = ((a << 8) | SIGN) >> (31 - e);
#endif
    return (a & SIGN) ? (int32_t)(0u - m) : (int32_t)m;
}

int32_t SF_NAME(fixsfsi)(sf_float a) { return to_i32_bits(f2b(a)); }
