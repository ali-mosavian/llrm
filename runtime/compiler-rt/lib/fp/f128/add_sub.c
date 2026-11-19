/* __addtf3, __subtf3. */
#include "f128.h"
#include "../add.inc"

sf_quad SF_NAME(addtf3)(sf_quad a, sf_quad b) { return b2q(add_bits_128(q2b(a), q2b(b))); }
sf_quad SF_NAME(subtf3)(sf_quad a, sf_quad b) { return b2q(sub_bits_128(q2b(a), q2b(b))); }

#include "../core_end.inc"
