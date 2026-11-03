/* __floatsisf; no 64-bit type, so every target can build it. */
#include "float_si.h"

static sf32 from_i32_bits(int32_t x) { return x < 0 ? from_mag(SIGN, 0u - (uint32_t)x) : from_mag(0, (uint32_t)x); }

sf_float SF_NAME(floatsisf)(int32_t x) { return b2f(from_i32_bits(x)); }
