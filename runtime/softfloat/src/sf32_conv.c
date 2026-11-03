/* binary32 <-> 32-bit integer; no 64-bit type, so every target can build it. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

/* Round to nearest even and pack (see softfloat.h). */
sf32 sf32_pack(uint32_t sign, int e, uint32_t m)
{
    if (e >= 255) return sign | 0x7F800000u;
    if (e <= 0) { m = sf_shr_sticky32(m, 1 - e); e = 1; }
    m = sf_round3(m);
    return sign + ((uint32_t)(e - 1) << 23) + m;
}

static sf32 from_mag(uint32_t sign, uint32_t x)
{
    if (!x) return 0;
    int n = 31 - sf_clz32(x);
    uint32_t m = n > 26 ? sf_shr_sticky32(x, n - 26) : x << (26 - n);
    return pack_round(sign, n + 127, m); /* n <= 31: e is 127..158 */
}

static sf32 from_u32_bits(uint32_t x) { return from_mag(0, x); }
static sf32 from_i32_bits(int32_t x) { return x < 0 ? from_mag(SIGN, 0u - (uint32_t)x) : from_mag(0, (uint32_t)x); }

/* |a| truncated; *big is set from 2^32 up, where the result saturates. */
static int32_t to_i32_bits(sf32 a)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    if (e < 0) return 0;
    if (e >= 31) { /* from 2^31 up: saturates; -2^31 is the saturated value itself */
        if ((a << 1) > 0xFF000000u) return 0;
        return (a & SIGN) ? INT32_MIN : INT32_MAX;
    }
    uint32_t m = (a & 0x7FFFFFu) | 0x800000u;
    m = e >= 23 ? m << (e - 23) : m >> (23 - e);
    return (a & SIGN) ? (int32_t)(0u - m) : (int32_t)m;
}

static uint32_t to_u32_bits(sf32 a)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    if (e < 0 || (a & SIGN)) return 0;
    if (e >= 32) return (a << 1) > 0xFF000000u ? 0 : UINT32_MAX;
    uint32_t m = (a & 0x7FFFFFu) | 0x800000u;
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}

sf_float SF_NAME(floatsisf)(int32_t x) { return b2f(from_i32_bits(x)); }
sf_float SF_NAME(floatunsisf)(uint32_t x) { return b2f(from_u32_bits(x)); }
int32_t SF_NAME(fixsfsi)(sf_float a) { return to_i32_bits(f2b(a)); }
uint32_t SF_NAME(fixunssfsi)(sf_float a) { return to_u32_bits(f2b(a)); }
