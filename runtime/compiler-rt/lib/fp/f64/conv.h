/* Shared by the binary64 integer conversions: truncated magnitude, saturating signed and unsigned results, and integer to binary64. */
#include "f64.h"

#ifndef SF_CONV64_H
#define SF_CONV64_H
/* |a| truncated; *big is set from 2^bits up, where the result saturates. */
static inline uint64_t magnitude(sf64 a, int bits, int *big)
{
    int e = (int)((a >> 52) & 0x7FF) - 1023;
    *big = 0;
    if (e < 0) return 0;
    if (e >= bits) { *big = 1; return 0; }
    uint64_t m = (a & (SF_HID - 1)) | SF_HID;
    return e >= 52 ? m << (e - 52) : m >> (52 - e);
}

static inline int64_t to_signed(sf64 a, int bits)
{
    int big;
    uint64_t m = magnitude(a, bits, &big), top = 1ull << (bits - 1);
    if (is_nan_64(a)) return 0;
    if (a & SF_SIGN) return (big || m > top) ? (int64_t)(0ull - top) : (int64_t)(0ull - m);
    return (big || m > top - 1) ? (int64_t)(top - 1) : (int64_t)m;
}

static inline uint64_t to_unsigned(sf64 a, int bits)
{
    int big;
    uint64_t m = magnitude(a, bits, &big);
    if (is_nan_64(a) || (a & SF_SIGN)) return 0;
    return big ? (bits == 64 ? UINT64_MAX : (1ull << bits) - 1) : m;
}

static inline sf64 from_mag(uint64_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint64_t m = n > 55 ? sf_shr_sticky64(u, n - 55) : u << (55 - n);
    return SF_PACK(sign, n + 1023, m);
}
#endif
