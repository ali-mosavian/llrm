/* IEEE-754 binary32 (round to nearest even, denormals, inf, NaN) shaped for an 8-bit CPU with a 4-byte-register
 * backend. Replaces sf32.c + sf32_conv.c. The tricks, several from the Rankin/Wozniak routines:
 *  - mantissas live left-aligned in a uint32: 24 bits + one guard byte, so rounding reads one byte and every
 *    shift by 8 is a byte move; the only shifts are constants 1 and 8 (inline ASL/ROL chains), never a variable one;
 *  - alignment and normalization move whole bytes first, then single bits (at most 7);
 *  - the multiplier is a shift-add over a 24+24 bit product (the Woz M1:E scheme), one test and add per bit;
 *  - the divider is a restoring subtract-compare loop, one quotient bit per step, remainder as the sticky bit;
 *  - the sign is carried as a parity, the magnitude is compared as a pattern, the exponent is a byte.
 * Rare paths (inf, NaN, zero, huge exponent gaps) are out of line. */
#include "softfloat.h"
#include "sf_helpers.h"

typedef uint32_t u32;
typedef uint8_t u8;
#define SIGN 0x80000000u
#define INF 0x7F800000u
#define NOINLINE __attribute__((noinline))

/* s: implicit one at bit 31 (or clear when e == 1: denormal), guard byte below; e: biased, may be <= 0 or >= 255. */
static sf32 finish(u32 sign, int e, u32 s)
{
    if (e >= 255) return sign | INF;
    if (e <= 0) {                       /* denormal: shift right with sticky */
        int n = 1 - e;
        u32 st = 0;
        if (n >= 32) return sign | 0;   /* below half the smallest denormal; s != 0 is below 2^-150 here */
        for (; n >= 8; n -= 8) { st |= (u8)s; s >>= 8; }
        for (; n; n--) { st |= s & 1; s >>= 1; }
        s |= st != 0;
        e = 1;
    }
    u32 m = s >> 8;
    u8 g = (u8)s;
    if (g > 0x80 || (g == 0x80 && (m & 1))) m++;
    return sign + ((u32)(e - 1) << 23) + m;
}

#ifndef SF_EXT_MULDIV
/* The adapter the 64-bit conversions use: three guard bits, implicit one at bit 26. */
sf32 sf32_pack(u32 sign, int e, u32 m) { return finish(sign, e, m << 5); }

#endif
static NOINLINE sf32 nan_of(sf32 a, sf32 b) { return ((a << 1) > (INF << 1) ? a : b) | 0x400000u; }

/* ---- add / sub ---- */

#ifndef SF_EXT_MULDIV
static NOINLINE sf32 add_special(sf32 a, sf32 b)
{
    if ((a << 1) > (INF << 1) || (b << 1) > (INF << 1)) return nan_of(a, b);
    if ((a << 1) == (INF << 1)) return ((b << 1) == (INF << 1) && ((a ^ b) & SIGN)) ? SF32_QNAN : a;
    if ((b << 1) == (INF << 1)) return b;
    if (!(a << 1)) return (b << 1) ? b : (a & b);
    return a;                           /* b is zero */
}

#endif
static u32 unpack24(sf32 a, int *e)     /* 24-bit mantissa with the implicit one; denormals have e = 1 and none */
{
    u8 hi = (u8)(a >> 24), mid = (u8)(a >> 16);
    int x = (u8)(hi << 1) | (mid >> 7);
    u32 m = a & 0x7FFFFFu;
    if (x) m |= 0x800000u; else x = 1;
    *e = x;
    return m;
}

#ifndef SF_EXT_MULDIV
static u32 unpack(sf32 a, int *e) { return unpack24(a, e) << 8; }

static sf32 add_bits(sf32 a, sf32 b)
{
    if ((a << 1) >= INF << 1 || !(b << 1) || (b << 1) >= INF << 1 || !(a << 1)) return add_special(a, b);
    if ((a << 1) < (b << 1)) { sf32 t = a; a = b; b = t; }
    int e, eb;
    u32 ma = unpack(a, &e), mb = unpack(b, &eb), sign = a & SIGN;
    int d = e - eb;
    if (d) {                            /* align the smaller operand: bytes, then bits */
        if (d >= 32) mb = 1;
        else {
            u32 st = 0;
            for (; d >= 8; d -= 8) { st |= (u8)mb; mb >>= 8; }
            for (; d; d--) { st |= mb & 1; mb >>= 1; }
            mb |= st != 0;
        }
    }
    u32 s;
    if ((a ^ b) & SIGN) {
        s = ma - mb;
        if (!s) return 0;
        while (e > 8 && !(s >> 24)) { s <<= 8; e -= 8; }
        while (e > 1 && !(s & SIGN)) { s <<= 1; e--; }
    } else {
        s = ma + mb;
        if (s < ma) { s = (s >> 1) | SIGN | (s & 1); e++; }
    }
    return finish(sign, e, s);
}

sf_float SF_NAME(addsf3)(sf_float a, sf_float b) { return b2f(add_bits(f2b(a), f2b(b))); }
sf_float SF_NAME(subsf3)(sf_float a, sf_float b) { return b2f(add_bits(f2b(a), f2b(b) ^ SIGN)); }
sf_float SF_NAME(negsf2)(sf_float a) { return b2f(f2b(a) ^ SIGN); }

#endif
/* ---- mul ---- */

static NOINLINE sf32 mul_special(sf32 a, sf32 b)
{
    u32 sign = (a ^ b) & SIGN;
    if ((a << 1) > (INF << 1) || (b << 1) > (INF << 1)) return nan_of(a, b);
    if (!(a << 1) || !(b << 1)) return ((a << 1) == (INF << 1) || (b << 1) == (INF << 1)) ? SF32_QNAN : sign;
    return sign | INF;
}

static sf32 mul_bits(sf32 a, sf32 b)
{
    if ((a << 1) >= INF << 1 || !(a << 1) || (b << 1) >= INF << 1 || !(b << 1)) return mul_special(a, b);
    int ea, eb;
    u32 x = unpack24(a, &ea), y = unpack24(b, &eb), sign = (a ^ b) & SIGN;
    while (!(x & 0x800000u)) { x <<= 1; ea--; }
    while (!(y & 0x800000u)) { y <<= 1; eb--; }
    u32 hi = 0, lo = y;
    for (u8 i = 24; i; i--) {           /* 48-bit product in hi:lo, 24 bits each, one multiplier bit per step */
        if (lo & 1) hi += x;
        lo = (lo >> 1) | ((hi & 1) << 23);
        hi >>= 1;
    }
    int e = ea + eb - 127;
    if (!(hi & 0x800000u)) { hi = (hi << 1) | (lo >> 23); lo = (lo << 1) & 0xFFFFFFu; e--; }
    /* mantissa = hi (24 bits), guard byte = lo's top byte, sticky = the rest of lo */
    u32 s = (hi << 8) | (lo >> 16) | ((lo & 0xFFFFu) != 0);
    return finish(sign, e + 1, s);
}

sf_float SF_NAME(mulsf3)(sf_float a, sf_float b) { return b2f(mul_bits(f2b(a), f2b(b))); }

/* ---- div ---- */

static NOINLINE sf32 div_special(sf32 a, sf32 b)
{
    u32 sign = (a ^ b) & SIGN;
    if ((a << 1) > (INF << 1) || (b << 1) > (INF << 1)) return nan_of(a, b);
    if ((a << 1) == (INF << 1)) return (b << 1) == (INF << 1) ? SF32_QNAN : sign | INF;
    if ((b << 1) == (INF << 1)) return sign;
    if (!(b << 1)) return !(a << 1) ? SF32_QNAN : sign | INF;
    return sign;                        /* a is zero */
}

static sf32 div_bits(sf32 a, sf32 b)
{
    if ((a << 1) >= INF << 1 || !(a << 1) || (b << 1) >= INF << 1 || !(b << 1)) return div_special(a, b);
    int ea, eb;
    u32 x = unpack24(a, &ea), y = unpack24(b, &eb), sign = (a ^ b) & SIGN, q = 0;
    while (!(x & 0x800000u)) { x <<= 1; ea--; }
    while (!(y & 0x800000u)) { y <<= 1; eb--; }
    for (u8 i = 28; i; i--) {           /* q = floor(x * 2^27 / y), one bit per step */
        q <<= 1;
        if (x >= y) { x -= y; q |= 1; }
        x <<= 1;
    }
    int e = ea - eb + 126;
    if (q >> 27) { q = (q >> 1) | (q & 1); e++; }
    return finish(sign, e, (q << 5) | (x != 0));
}

sf_float SF_NAME(divsf3)(sf_float a, sf_float b) { return b2f(div_bits(f2b(a), f2b(b))); }

#ifndef SF_EXT_MULDIV
/* ---- integer conversions ---- */

static sf32 from_mag(u32 sign, u32 x)
{
    if (!x) return 0;
    int e = 158;                        /* 127 + 31 */
    while (!(x >> 24)) { x <<= 8; e -= 8; }
    while (!(x & SIGN)) { x <<= 1; e--; }
    return finish(sign, e, x);
}

sf_float SF_NAME(floatunsisf)(uint32_t x) { return b2f(from_mag(0, x)); }
sf_float SF_NAME(floatsisf)(int32_t x) { return b2f(x < 0 ? from_mag(SIGN, 0u - (u32)x) : from_mag(0, (u32)x)); }

static u32 mag(sf32 a, int *big)        /* |a| truncated; *big from 2^32 up */
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    *big = 0;
    if (e < 0) return 0;
    if (e >= 32) { *big = 1; return 0; }
    u32 m = (a & 0x7FFFFFu) | 0x800000u;
    if (e >= 23) { int n = e - 23; for (; n >= 8; n -= 8) m <<= 8; for (; n; n--) m <<= 1; }
    else { int n = 23 - e; for (; n >= 8; n -= 8) m >>= 8; for (; n; n--) m >>= 1; }
    return m;
}

int32_t SF_NAME(fixsfsi)(sf_float fa)
{
    sf32 a = f2b(fa);
    int big;
    u32 m = mag(a, &big);
    if ((a << 1) > (INF << 1)) return 0;
    if (a & SIGN) return (big || m > SIGN) ? INT32_MIN : (int32_t)(0u - m);
    return (big || m > 0x7FFFFFFFu) ? INT32_MAX : (int32_t)m;
}

uint32_t SF_NAME(fixunssfsi)(sf_float fa)
{
    sf32 a = f2b(fa);
    int big;
    u32 m = mag(a, &big);
    if ((a << 1) > (INF << 1) || (a & SIGN)) return 0;
    return big ? UINT32_MAX : m;
}

/* ---- compare (unchanged contract: unordered is 1 for eq/ne/lt/le, -1 for ge/gt) ---- */

static int cmp_bits(sf32 a, sf32 b)
{
    if ((a << 1) > (INF << 1) || (b << 1) > (INF << 1)) return 2;
    if (!((a | b) << 1)) return 0;
    int32_t x = (int32_t)a, y = (int32_t)b;
    if (x < 0) x = (int32_t)SIGN - x;
    if (y < 0) y = (int32_t)SIGN - y;
    return x < y ? -1 : x > y;
}

int SF_NAME(cmpsf2)(sf_float a, sf_float b) { int c = cmp_bits(f2b(a), f2b(b)); return c == 2 ? 1 : c; }
int SF_NAME(eqsf2)(sf_float a, sf_float b) { return SF_NAME(cmpsf2)(a, b); }
int SF_NAME(nesf2)(sf_float a, sf_float b) { return SF_NAME(cmpsf2)(a, b); }
int SF_NAME(ltsf2)(sf_float a, sf_float b) { return SF_NAME(cmpsf2)(a, b); }
int SF_NAME(lesf2)(sf_float a, sf_float b) { return SF_NAME(cmpsf2)(a, b); }
int SF_NAME(gesf2)(sf_float a, sf_float b) { int c = cmp_bits(f2b(a), f2b(b)); return c == 2 ? -1 : c; }
int SF_NAME(gtsf2)(sf_float a, sf_float b) { return SF_NAME(gesf2)(a, b); }
int SF_NAME(unordsf2)(sf_float a, sf_float b) { return cmp_bits(f2b(a), f2b(b)) == 2; }
#endif
