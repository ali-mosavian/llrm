/* __addsf3, __subsf3. */
#include "f32.h"
#include "../add.inc"

sf_float SF_NAME(addsf3)(sf_float a, sf_float b) { return b2f(add_bits(f2b(a), f2b(b))); }
sf_float SF_NAME(subsf3)(sf_float a, sf_float b) { return b2f(sub_bits(f2b(a), f2b(b))); }

#include "../core_end.inc"
