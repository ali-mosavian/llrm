/* binary32 width header: the constants and decode/encode of the width-generic core (../macros.inc, ../codec.inc). A file that uses the core includes
 * this first and ends with ../core_end.inc, so another width can follow in the same translation unit. Working form: 27-bit mantissa (implicit one
 * at bit 26, three guard bits). */
#include "softfloat.h"
#include "sf_helpers.h"

#define SF_N(n) n
#define SF_T uint32_t
#define SF_S int32_t
#define SF_W 32
#define SF_MB 23
#define SF_EXPMAX 0xFF
#define SF_QNAN SF32_QNAN
#define sf_clz sf_clz32
#define sf_shr_sticky sf_shr_sticky32
#define SF_ROUND3 sf_round3
#define SF_PACK pack_near
#define SF_UNPACK SF_N(unpack)
#include "../macros.inc"
#ifndef SF_CODEC32
#define SF_CODEC32
#include "../codec.inc"
#endif
