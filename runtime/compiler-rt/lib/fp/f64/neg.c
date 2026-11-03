/* __negdf2. */
#include "f64.h"

sf_double SF_NAME(negdf2)(sf_double a) { return b2d(d2b(a) ^ SF_SIGN); }

#include "../core_end.inc"
