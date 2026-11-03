/* __unorddf2: only the NaN test, in its own file so a link that needs an ordering compare does not carry it and one that needs this does not carry the ordering core. */
#include "f64.h"

int SF_NAME(unorddf2)(sf_double a, sf_double b) { return is_nan_64(d2b(a)) || is_nan_64(d2b(b)); }

#include "../core_end.inc"
