/* Decode a binary64: mantissa with its implicit one, and the exponent (denormals get e = 1). */
#include "f64.h"

uint64_t sf64_unpack(uint64_t a, int *e) { return unpack_64(a, e); }

#include "../core_end.inc"
