/* __adddf3, __subdf3. */
#include "f64.h"
#include "../add.inc"

sf_double SF_NAME(adddf3)(sf_double a, sf_double b) { return b2d(add_bits_64(d2b(a), d2b(b))); }
sf_double SF_NAME(subdf3)(sf_double a, sf_double b) { return b2d(sub_bits_64(d2b(a), d2b(b))); }

#include "../core_end.inc"
