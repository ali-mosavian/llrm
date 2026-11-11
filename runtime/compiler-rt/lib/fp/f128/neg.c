/* __negtf2. */
#include "f128.h"

sf_quad SF_NAME(negtf2)(sf_quad a) { return b2q(q2b(a) ^ SF_SIGN); }

#include "../core_end.inc"
