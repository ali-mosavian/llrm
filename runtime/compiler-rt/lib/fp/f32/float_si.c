/* __floatsisf, __floatunsisf; no 64-bit type, so every target can build it. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

static sf32 from_mag(uint32_t sign, uint32_t x)
{
    if (!x) return 0;
    int n = 31 - sf_clz32(x);
    uint32_t m = n > 26 ? sf_shr_sticky32(x, n - 26) : x << (26 - n);
    return pack_round(sign, n + 127, m); /* n <= 31: e is 127..158 */
}

static sf32 from_u32_bits(uint32_t x) { return from_mag(0, x); }
static sf32 from_i32_bits(int32_t x) { return x < 0 ? from_mag(SIGN, 0u - (uint32_t)x) : from_mag(0, (uint32_t)x); }

sf_float SF_NAME(floatsisf)(int32_t x) { return b2f(from_i32_bits(x)); }
sf_float SF_NAME(floatunsisf)(uint32_t x) { return b2f(from_u32_bits(x)); }
