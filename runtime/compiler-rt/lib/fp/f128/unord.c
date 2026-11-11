/* __unordtf2: only the NaN test, in its own file so a link that needs an ordering compare does not carry it and one that needs this does not carry the ordering core. */
#include "f128.h"

int SF_NAME(unordtf2)(sf_quad a, sf_quad b) { return is_nan_128(q2b(a)) || is_nan_128(q2b(b)); }

#include "../core_end.inc"
