/* IEEE-754 binary64, round to nearest even, denormals, inf and NaN; the binary32 core (sf32.c) at twice the width.
 * Working form: 56-bit mantissa (implicit one at bit 55, three guard bits). Needs a 64-bit type. */
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x8000000000000000ull
#define INF 0x7FF0000000000000ull
#define QNAN 0x7FF8000000000000ull
#define HID 0x0010000000000000ull
#define LOW32 0xFFFFFFFFull

static inline int is_nan(sf64 a) { return (a << 1) > (INF << 1); }

/* Round to nearest even and pack: m has three guard bits and its implicit one at bit 55; e may be <= 0 or >= 2047. */
static sf64 pack(uint64_t sign, int e, uint64_t m)
{
    if (e >= 2047) return sign | INF;
    if (e <= 0) { m = sf_shr_sticky64(m, 1 - e); e = 1; }
    uint64_t r = m & 7;
    m >>= 3;
    if (r > 4 || (r == 4 && (m & 1))) m++;
    return sign + ((uint64_t)(e - 1) << 52) + m;
}

/* Mantissa and exponent; denormals get e = 1, no implicit bit. */
static inline uint64_t unpack(sf64 a, int *e)
{
    uint64_t m = a & (HID - 1);
    int x = (int)((a >> 52) & 0x7FF);
    if (x) m |= HID; else x = 1;
    *e = x;
    return m;
}

/* 64x64 -> 128 as hi:lo from 32x32 partial products. */
static void mul64(uint64_t a, uint64_t b, uint64_t *hi, uint64_t *lo)
{
    uint32_t h, l;
    uint64_t ll, lh, hl, hh, mid;
    sf_mul32x32((uint32_t)a, (uint32_t)b, &h, &l); ll = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)a, (uint32_t)(b >> 32), &h, &l); lh = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)(a >> 32), (uint32_t)b, &h, &l); hl = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)(a >> 32), (uint32_t)(b >> 32), &h, &l); hh = (uint64_t)h << 32 | l;
    mid = (ll >> 32) + (lh & LOW32) + (hl & LOW32);
    *lo = (ll & LOW32) | (mid << 32);
    *hi = hh + (lh >> 32) + (hl >> 32) + (mid >> 32);
}

static sf64 add_bits(sf64 a, sf64 b)
{
    if (((a << 1) >= (INF << 1)) | ((b << 1) >= (INF << 1))) {
        if (is_nan(a)) return a | (HID >> 1);
        if (is_nan(b)) return b | (HID >> 1);
        if ((a << 1) == (INF << 1) && (b << 1) == (INF << 1) && ((a ^ b) & SIGN)) return QNAN;
        return (a << 1) == (INF << 1) ? a : b;
    }
    if ((a << 1) == 0 || (b << 1) == 0) {
        if ((a << 1) == 0 && (b << 1) == 0) return a & b;
        return (a << 1) == 0 ? b : a;
    }
    if ((a & ~SIGN) < (b & ~SIGN)) { sf64 t = a; a = b; b = t; }
    int ea, eb;
    uint64_t ma = unpack(a, &ea) << 3, mb = unpack(b, &eb) << 3;
    if (ea - eb > 54) return a; /* ed > 54: b is under a quarter of a's last place; 54 is not enough (2^k - 1.5 * 2^(k-54) rounds down) */
    mb = sf_shr_sticky64(mb, ea - eb);
    uint64_t sign = a & SIGN, m;
    if ((a ^ b) & SIGN) {
        m = ma - mb;
        if (!m) return 0;
        int s = sf_clz64(m) - 8;
        if (s > ea - 1) s = ea - 1;
        m <<= s;
        ea -= s;
    } else {
        m = ma + mb;
        if (m & (1ull << 56)) { m = sf_shr_sticky64(m, 1); ea++; }
    }
    return pack(sign, ea, m);
}

static sf64 mul_bits(sf64 a, sf64 b)
{
    uint64_t sign = (a ^ b) & SIGN;
    if (((a << 1) >= (INF << 1)) | ((b << 1) >= (INF << 1))) {
        if (is_nan(a)) return a | (HID >> 1);
        if (is_nan(b)) return b | (HID >> 1);
        if ((a << 1) == 0 || (b << 1) == 0) return QNAN;
        return sign | INF;
    }
    if ((a << 1) == 0 || (b << 1) == 0) return sign;
    int ea, eb;
    uint64_t ma = unpack(a, &ea), mb = unpack(b, &eb);
    if (!(ma & HID)) { int s = sf_clz64(ma) - 11; ma <<= s; ea -= s; }
    if (!(mb & HID)) { int s = sf_clz64(mb) - 11; mb <<= s; eb -= s; }
    uint64_t hi, lo;
    mul64(ma, mb, &hi, &lo);
    /* The product has its top bit at 105 or 104; keep 56 bits and fold the rest. */
    int e = ea + eb - 1023, sh = 49;
    if (hi >> 41) { sh = 50; e++; }
    uint64_t m = (hi << (64 - sh)) | (lo >> sh) | ((lo << (64 - sh)) != 0);
    return pack(sign, e, m);
}

static sf64 div_bits(sf64 a, sf64 b)
{
    uint64_t sign = (a ^ b) & SIGN;
    if (((a << 1) >= (INF << 1)) | ((b << 1) >= (INF << 1))) {
        if (is_nan(a)) return a | (HID >> 1);
        if (is_nan(b)) return b | (HID >> 1);
        if ((a << 1) == (INF << 1)) return (b << 1) == (INF << 1) ? QNAN : sign | INF;
        return sign;
    }
    if ((b << 1) == 0) return (a << 1) == 0 ? QNAN : sign | INF;
    if ((a << 1) == 0) return sign;
    int ea, eb;
    uint64_t ma = unpack(a, &ea), mb = unpack(b, &eb);
    if (!(ma & HID)) { int s = sf_clz64(ma) - 11; ma <<= s; ea -= s; }
    if (!(mb & HID)) { int s = sf_clz64(mb) - 11; mb <<= s; eb -= s; }
    /* q = floor(ma * 2^56 / mb), one quotient bit per step. */
    uint64_t q = 0, r = ma;
    for (int i = 0; i < 57; i++) {
        q <<= 1;
        if (r >= mb) { r -= mb; q |= 1; }
        r <<= 1;
    }
    int e = ea - eb + 1022;
    if (q >> 56) { q = (q >> 1) | (q & 1); e++; }
    return pack(sign, e, q | (r != 0));
}

static int cmp_bits(sf64 a, sf64 b)
{
    if (is_nan(a) || is_nan(b)) return 2;
    if (!((a | b) << 1)) return 0;
    int64_t x = (int64_t)a, y = (int64_t)b;
    if (x < 0) x = (int64_t)SIGN - x;
    if (y < 0) y = (int64_t)SIGN - y;
    return x < y ? -1 : x > y;
}

/* |a| truncated; *big is set from 2^bits up, where the result saturates. */
static uint64_t magnitude(sf64 a, int bits, int *big)
{
    int e = (int)((a >> 52) & 0x7FF) - 1023;
    *big = 0;
    if (e < 0) return 0;
    if (e >= bits) { *big = 1; return 0; }
    uint64_t m = (a & (HID - 1)) | HID;
    return e >= 52 ? m << (e - 52) : m >> (52 - e);
}

static int64_t to_signed(sf64 a, int bits)
{
    int big;
    uint64_t m = magnitude(a, bits, &big), top = 1ull << (bits - 1);
    if (is_nan(a)) return 0;
    if (a & SIGN) return (big || m > top) ? (int64_t)(0ull - top) : (int64_t)(0ull - m);
    return (big || m > top - 1) ? (int64_t)(top - 1) : (int64_t)m;
}

static uint64_t to_unsigned(sf64 a, int bits)
{
    int big;
    uint64_t m = magnitude(a, bits, &big);
    if (is_nan(a) || (a & SIGN)) return 0;
    return big ? (bits == 64 ? UINT64_MAX : (1ull << bits) - 1) : m;
}

static sf64 from_mag(uint64_t sign, uint64_t u)
{
    if (!u) return 0;
    int n = 63 - sf_clz64(u);
    uint64_t m = n > 55 ? sf_shr_sticky64(u, n - 55) : u << (55 - n);
    return pack(sign, n + 1023, m);
}

sf_double SF_NAME(adddf3)(sf_double a, sf_double b) { return b2d(add_bits(d2b(a), d2b(b))); }
sf_double SF_NAME(subdf3)(sf_double a, sf_double b) { return b2d(add_bits(d2b(a), d2b(b) ^ SIGN)); }
sf_double SF_NAME(muldf3)(sf_double a, sf_double b) { return b2d(mul_bits(d2b(a), d2b(b))); }
sf_double SF_NAME(divdf3)(sf_double a, sf_double b) { return b2d(div_bits(d2b(a), d2b(b))); }
sf_double SF_NAME(negdf2)(sf_double a) { return b2d(d2b(a) ^ SIGN); }

/* compiler-rt contract: unordered is 1 for eq/ne/lt/le and -1 for ge/gt. */
int SF_NAME(cmpdf2)(sf_double a, sf_double b) { int c = cmp_bits(d2b(a), d2b(b)); return c == 2 ? 1 : c; }
int SF_NAME(eqdf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(nedf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(ltdf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(ledf2)(sf_double a, sf_double b) { return SF_NAME(cmpdf2)(a, b); }
int SF_NAME(gedf2)(sf_double a, sf_double b) { int c = cmp_bits(d2b(a), d2b(b)); return c == 2 ? -1 : c; }
int SF_NAME(gtdf2)(sf_double a, sf_double b) { return SF_NAME(gedf2)(a, b); }
int SF_NAME(unorddf2)(sf_double a, sf_double b) { return cmp_bits(d2b(a), d2b(b)) == 2; }

sf_double SF_NAME(floatsidf)(int32_t x) { return b2d(x < 0 ? from_mag(SIGN, 0ull - (uint64_t)(int64_t)x) : from_mag(0, (uint64_t)x)); }
sf_double SF_NAME(floatunsidf)(uint32_t x) { return b2d(from_mag(0, x)); }
sf_double SF_NAME(floatdidf)(int64_t x) { return b2d(x < 0 ? from_mag(SIGN, 0ull - (uint64_t)x) : from_mag(0, (uint64_t)x)); }
sf_double SF_NAME(floatundidf)(uint64_t x) { return b2d(from_mag(0, x)); }
int32_t SF_NAME(fixdfsi)(sf_double a) { return (int32_t)to_signed(d2b(a), 32); }
uint32_t SF_NAME(fixunsdfsi)(sf_double a) { return (uint32_t)to_unsigned(d2b(a), 32); }
int64_t SF_NAME(fixdfdi)(sf_double a) { return to_signed(d2b(a), 64); }
uint64_t SF_NAME(fixunsdfdi)(sf_double a) { return to_unsigned(d2b(a), 64); }
