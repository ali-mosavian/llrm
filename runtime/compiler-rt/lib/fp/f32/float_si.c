/* __floatsisf; no 64-bit type, so every target can build it. */
#include "float_si.h"

static sf32 from_i32_bits(int32_t x)
{
    uint32_t s = (uint32_t)(x >> 31); /* all ones for a negative x: one magnitude, one conversion */
    return from_mag(s & SIGN, ((uint32_t)x ^ s) - s);
}

sf_float SF_NAME(floatsisf)(int32_t x) { return b2f(from_i32_bits(x)); }
