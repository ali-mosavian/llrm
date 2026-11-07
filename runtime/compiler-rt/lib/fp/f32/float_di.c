/* __floatdisf, __floatundisf; needs a 64-bit type. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

#ifdef SF_CLZ64 /* n <= 63 drops at most 40 bits: round to nearest even at bit 23 with no pack call (f64's round_mag); a carry out of the significand lands in the exponent by the addition (the implicit bit is added with m) */
static sf32 from_mag(uint32_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint32_t m;
    if (n <= 23) m = (uint32_t)u << (23 - n);
    else {
        int k = n - 23;
        uint64_t r = u & ((1ull << k) - 1), h = 1ull << (k - 1);
        m = (uint32_t)(u >> k);
        m += (r > h) | ((r == h) & (m & 1));
    }
    return sign + ((uint32_t)(n + 126) << 23) + m;
}
#else
static sf32 from_mag(uint32_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint32_t m = n > 26 ? (uint32_t)sf_shr_sticky64(u, n - 26) : (uint32_t)u << (26 - n);
    return sf32_pack(sign, n + 127, m);
}
#endif

static sf32 from_u64_bits(uint64_t x) { return from_mag(0, x); }
#ifdef SF_CLZ64 /* one body: a branch on the sign duplicated the normaliser */
static sf32 from_i64_bits(int64_t x) { uint64_t s = (uint64_t)(x >> 63); return from_mag((uint32_t)s & SIGN, ((uint64_t)x ^ s) - s); }
#else
static sf32 from_i64_bits(int64_t x) { return x < 0 ? from_mag(SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x); }
#endif

sf_float SF_NAME(floatdisf)(int64_t x) { return b2f(from_i64_bits(x)); }
sf_float SF_NAME(floatundisf)(uint64_t x) { return b2f(from_u64_bits(x)); }
