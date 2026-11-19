/* __negsf2. */
#include "f32.h"

sf_float SF_NAME(negsf2)(sf_float a) { return b2f(f2b(a) ^ SF_SIGN); }

#include "../core_end.inc"
