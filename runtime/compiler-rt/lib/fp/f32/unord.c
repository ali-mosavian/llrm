/* __unordsf2: only the NaN test, in its own file so a link that needs an ordering compare does not carry it and one that needs this does not carry the ordering core. */
#include "f32.h"

int SF_NAME(unordsf2)(sf_float a, sf_float b) { return is_nan(f2b(a)) || is_nan(f2b(b)); }

#include "../core_end.inc"
