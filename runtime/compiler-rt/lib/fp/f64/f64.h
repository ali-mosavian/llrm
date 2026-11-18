/* binary64 width header: the binary32 core at twice the width (see f32/f32.h). Working form: 56-bit mantissa (implicit one at bit 55, three guard
 * bits). Needs a 64-bit type. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SF_N(n) n##_64
#define SF_T uint64_t
#define SF_S int64_t
#define SF_W 64
#define SF_MB 52
#define SF_EXPMAX 0x7FF
#define SF_QNAN 0x7FF8000000000000ull
#define sf_clz sf_clz64
#define sf_shr_sticky sf_shr_sticky64
#define SF_SHR_STICKY_ADD sf_shr_sticky64
#define SF_ROUND3 sf_round3_64
#define SF_PACK sf64_pack
#ifdef SF_UNPACK_SHARED /* one decode in unpack.c for every operation, as sf64_pack is for the encode */
uint64_t sf64_unpack(uint64_t a, int *e);
#define SF_UNPACK sf64_unpack
#else
#define SF_UNPACK SF_N(unpack)
#endif
#include "../macros.inc"
/* Encoding is a shared function (pack.c), as sf32_pack is: an inline copy in every operation costs more than the call. */
uint64_t sf64_pack(uint64_t sign, int e, uint64_t m);
#ifndef SF_CODEC64
#define SF_CODEC64
#include "../codec.inc"
#endif
