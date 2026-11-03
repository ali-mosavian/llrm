/* Spike: fast 4-byte float in the Rankin/Woz layout, written from the algorithm's description (align, add two's
 * complement mantissas, normalize; shift-add multiply; restoring divide), not from its listing.
 * Word = (M << 8) | E: E the biased exponent, M a 24-bit two's complement mantissa, value = M / 2^22 * 2^(E - 128),
 * normalized when mantissa bits 23 and 22 differ. Truncating. No denormals, inf or NaN: underflow gives 0, overflow
 * and division by zero saturate. 32-bit operations only. */
#include <stdint.h>

#ifndef FZ_NAME
#define FZ_NAME(n) fz_##n
#endif

typedef uint32_t fz;

#define MAXMAG 0x7FFFFF

static inline int32_t mant(fz w) { return (int32_t)w >> 8; }
static inline int expo(fz w) { return (int)(w & 255); }
static inline fz pack(int e, int32_t m) { return ((uint32_t)m << 8) | (uint32_t)e; }
static fz saturate(int32_t sign) { return pack(255, sign ? -0x800000 : MAXMAG); }

/* Shift left until bits 23 and 22 differ; a zero mantissa is the zero word. */
static fz norm(int e, int32_t m)
{
    if (!m) return 0;
    while (!((m ^ (m << 1)) & 0x800000)) { m <<= 1; e--; }   /* bits 23 and 22 equal */
    if (e < 0) return 0;
    if (e > 255) return saturate(m < 0);
    return pack(e, m);
}

fz FZ_NAME(add)(fz a, fz b)
{
    int ea = expo(a), eb = expo(b);
    int32_t ma = mant(a), mb = mant(b);
    if (ea < eb) { int t = ea; ea = eb; eb = t; int32_t u = ma; ma = mb; mb = u; }
    if (!ma) return pack(eb, mb);
    int d = ea - eb;
    if (d > 23) return pack(ea, ma);
    int32_t m = ma + (mb >> d);
    if (m > MAXMAG || m < -0x800000) { m >>= 1; ea++; }
    return norm(ea, m);
}

fz FZ_NAME(neg)(fz a)
{
    int e = expo(a);
    int32_t m = mant(a);
    if (m == -0x800000) { m = -0x400000; e++; }
    return norm(e, -m);
}

fz FZ_NAME(sub)(fz a, fz b) { return FZ_NAME(add)(a, FZ_NAME(neg)(b)); }

/* |m| as a normalized unsigned mantissa in [2^22, 2^23). */
static uint32_t magnitude(int32_t m, int *e)
{
    uint32_t u = m < 0 ? (uint32_t)-m : (uint32_t)m;
    if (u > MAXMAG) { u >>= 1; (*e)++; }
    return u;
}

fz FZ_NAME(mul)(fz a, fz b)
{
    int32_t ma = mant(a), mb = mant(b);
    if (!ma || !mb) return 0;
    int e = expo(a) + expo(b) - 128, sign = (ma < 0) != (mb < 0);
    uint32_t x = magnitude(ma, &e), y = magnitude(mb, &e), hi = 0, lo = y;
    for (int i = 0; i < 24; i++) {       /* 48-bit product in hi:lo, one multiplier bit per step */
        if (lo & 1) hi += x;
        lo = (lo >> 1) | ((hi & 1) << 23);   /* 24-bit halves: no 32-bit funnel shift */
        hi >>= 1;
    }
    uint32_t p = (hi << 2) | (lo >> 22);   /* product bits 47..22: hi is bits 47..24, lo bits 23..0 */
    if (p > MAXMAG) { p >>= 1; e++; }
    return norm(e, sign ? -(int32_t)p : (int32_t)p);
}

fz FZ_NAME(div)(fz a, fz b)   /* a / b */
{
    int32_t ma = mant(a), mb = mant(b);
    if (!mb) return saturate(ma < 0);
    if (!ma) return 0;
    int e = expo(a) - expo(b) + 128, sign = (ma < 0) != (mb < 0);
    int ex = 0, ey = 0;
    uint32_t x = magnitude(ma, &ex), y = magnitude(mb, &ey), q = 0, r = x;
    e += ex - ey;                        /* a divisor that was halved to normalize lowers the quotient's exponent */
    for (int i = 0; i < 24; i++) {       /* q = floor(x * 2^23 / y) */
        q <<= 1;
        if (r >= y) { r -= y; q |= 1; }
        r <<= 1;
    }
    if (q > MAXMAG) q >>= 1; else e--;
    return norm(e, sign ? -(int32_t)q : (int32_t)q);
}

fz FZ_NAME(from_i16)(int16_t n) { return norm(0x8E, (int32_t)n * 256); }

int16_t FZ_NAME(to_i16)(fz a)
{
    int e = expo(a);
    int32_t m = mant(a);
    if (e < 128) return 0;
    int s = 0x8E - e;
    if (s < 0) return m < 0 ? -32768 : 32767;
    uint32_t u = (m < 0 ? (uint32_t)-m : (uint32_t)m) >> (s + 8);   /* toward zero */
    if (u > 32767) return m < 0 ? -32768 : 32767;
    return (int16_t)(m < 0 ? -(int32_t)u : (int32_t)u);
}
