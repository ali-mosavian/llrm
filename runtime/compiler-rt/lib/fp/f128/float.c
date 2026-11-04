/* __floatsitf, __floatunsitf, __floatditf, __floatunditf: exact (at most 64 significant bits). */
#include "conv.h"

sf_quad SF_NAME(floatsitf)(int32_t x) { return b2q(x < 0 ? from_mag(SF_SIGN, (sf128)(0ull - (uint64_t)(int64_t)x)) : from_mag(0, (sf128)(uint32_t)x)); }
sf_quad SF_NAME(floatunsitf)(uint32_t x) { return b2q(from_mag(0, x)); }
sf_quad SF_NAME(floatditf)(int64_t x) { return b2q(x < 0 ? from_mag(SF_SIGN, (sf128)(0ull - (uint64_t)x)) : from_mag(0, (sf128)(uint64_t)x)); }
sf_quad SF_NAME(floatunditf)(uint64_t x) { return b2q(from_mag(0, x)); }

#include "../core_end.inc"
