/* __floatdisf, __floatundisf; needs a 64-bit type. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

static sf32 from_mag(uint32_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint32_t m = n > 26 ? (uint32_t)sf_shr_sticky64(u, n - 26) : (uint32_t)u << (26 - n);
    return sf32_pack(sign, n + 127, m);
}

static sf32 from_u64_bits(uint64_t x) { return from_mag(0, x); }
static sf32 from_i64_bits(int64_t x) { return x < 0 ? from_mag(SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x); }

sf_float SF_NAME(floatdisf)(int64_t x) { return b2f(from_i64_bits(x)); }
sf_float SF_NAME(floatundisf)(uint64_t x) { return b2f(from_u64_bits(x)); }
