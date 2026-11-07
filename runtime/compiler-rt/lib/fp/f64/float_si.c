/* __floatsidf. Only with SF_CONV_SPLIT (a target that links whole objects: float.c holds all four otherwise). */
#include "conv.h"
#ifdef SF_CONV_SPLIT

#if defined(SF_CLZ64) || defined(SF_CONV_DIRECT32) /* the 32-bit conversions are exact and call neither pack nor the 64-bit path */
sf_double SF_NAME(floatsidf)(int32_t x) /* one body: a branch on the sign duplicated it */
{
    uint32_t s = (uint32_t)(x >> 31);
    return b2d(from_mag32((uint64_t)((uint32_t)x & 0x80000000u) << 32, ((uint32_t)x ^ s) - s));
}
#else
sf_double SF_NAME(floatsidf)(int32_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)(int64_t)x) : from_mag(0, (uint64_t)x)); }
#endif
#endif

#include "../core_end.inc"
