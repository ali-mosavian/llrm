/* __floatdidf. Only with SF_CONV_SPLIT (a target that links whole objects: float.c holds all four otherwise). */
#include "conv.h"
#ifdef SF_CONV_SPLIT

sf_double SF_NAME(floatdidf)(int64_t x) { return b2d(x < 0 ? from_mag(SF_SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x)); }
#endif

#include "../core_end.inc"
