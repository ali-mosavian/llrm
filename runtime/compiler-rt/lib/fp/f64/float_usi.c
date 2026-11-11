/* __floatunsidf. Only with SF_CONV_SPLIT (a target that links whole objects: float.c holds all four otherwise). */
#include "conv.h"
#ifdef SF_CONV_SPLIT

#if defined(SF_CLZ64) || defined(SF_CONV_DIRECT32)
sf_double SF_NAME(floatunsidf)(uint32_t x) { return b2d(from_mag32(0, x)); }
#else
sf_double SF_NAME(floatunsidf)(uint32_t x) { return b2d(from_mag(0, x)); }
#endif
#endif

#include "../core_end.inc"
