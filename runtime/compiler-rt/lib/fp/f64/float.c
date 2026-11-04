/* __floatsidf, __floatunsidf, __floatdidf, __floatundidf. */
#include "conv.h"

sf_double SF_NAME(floatsidf)(int32_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)(int64_t)x) : from_mag(0, (uint64_t)x)); }
sf_double SF_NAME(floatunsidf)(uint32_t x) { return b2d(from_mag(0, x)); }
sf_double SF_NAME(floatdidf)(int64_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x)); }
sf_double SF_NAME(floatundidf)(uint64_t x) { return b2d(from_mag(0, x)); }

#include "../core_end.inc"
