/* __floatsidf, __floatunsidf, __floatdidf, __floatundidf. */
#include "conv.h"

#ifdef SF_CLZ64 /* native clz: the 32-bit conversions are exact and call neither pack nor the 64-bit path */
sf_double SF_NAME(floatsidf)(int32_t x) /* one body: a branch on the sign duplicated it */
{
    uint32_t s = (uint32_t)(x >> 31);
    return b2d(from_mag32((uint64_t)s << 63, ((uint32_t)x ^ s) - s));
}
sf_double SF_NAME(floatunsidf)(uint32_t x) { return b2d(from_mag32(0, x)); }
#else
sf_double SF_NAME(floatsidf)(int32_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)(int64_t)x) : from_mag(0, (uint64_t)x)); }
sf_double SF_NAME(floatunsidf)(uint32_t x) { return b2d(from_mag(0, x)); }
#endif
sf_double SF_NAME(floatdidf)(int64_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x)); }
sf_double SF_NAME(floatundidf)(uint64_t x) { return b2d(from_mag(0, x)); }

#include "../core_end.inc"
