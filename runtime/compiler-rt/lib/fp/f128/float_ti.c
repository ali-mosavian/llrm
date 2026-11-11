/* __floattitf, __floatuntitf: 128-bit integers, rounded to nearest even. */
#include "conv.h"

sf_quad SF_NAME(floattitf)(__int128 x) { return b2q(x < 0 ? from_mag(SF_SIGN, (sf128)0 - (sf128)x) : from_mag(0, (sf128)x)); }
sf_quad SF_NAME(floatuntitf)(sf128 x) { return b2q(from_mag(0, x)); }

#include "../core_end.inc"
