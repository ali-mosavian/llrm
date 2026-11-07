/* __floatdidf. Only with SF_CONV_SPLIT (a target that links whole objects: float.c holds all four otherwise). */
#include "conv.h"
#ifdef SF_CONV_SPLIT

#ifdef SF_CLZ64 /* one body: a branch on the sign duplicated the 64-bit normaliser (riscv64 floatdidf 434 B, compiler-rt 270) */
sf_double SF_NAME(floatdidf)(int64_t x)
{
    uint64_t s = (uint64_t)(x >> 63);
    return b2d(from_mag(s & SF_SIGN, ((uint64_t)x ^ s) - s));
}
#else
sf_double SF_NAME(floatdidf)(int64_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x)); }
#endif
#endif

#include "../core_end.inc"
