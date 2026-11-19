/* __floatundidf. Only with SF_CONV_SPLIT (a target that links whole objects: float.c holds all four otherwise). */
#include "conv.h"
#ifdef SF_CONV_SPLIT

sf_double SF_NAME(floatundidf)(uint64_t x) { return b2d(from_mag(0, x)); }
#endif

#include "../core_end.inc"
