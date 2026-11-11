/* binary128 width header: the binary32 core at four times the width (see f32/f32.h). Working form: 116-bit mantissa (implicit one at bit 115, three
 * guard bits). Needs a 128-bit integer type (unsigned __int128). */
#include "softfloat.h"
#include "sf_helpers.h"

#define SF_N(n) n##_128
#define SF_T sf128
#define SF_S __int128
#define SF_W 128
#define SF_MB 112
#define SF_EXPMAX 0x7FFF
#define SF_QNAN (((sf128)0x7FFF8 << 108))
#define sf_clz sf_clz128
#define sf_shr_sticky sf_shr_sticky128
#define SF_SHR_STICKY_ADD sf_shr_sticky128
#define SF_ROUND3 sf_round3_128
#define SF_PACK sf128_pack
#define SF_UNPACK SF_N(unpack)
#include "../macros.inc"
/* Encoding is a shared function (pack.c), as sf32_pack is: an inline copy in every operation costs more than the call. */
sf128 sf128_pack(sf128 sign, int e, sf128 m);
#ifndef SF_CODEC128
#define SF_CODEC128
#include "../codec.inc"
#endif
