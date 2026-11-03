/* IEEE-style 8-bit float (sign, E exponent bits, M mantissa bits, E + M = 7; E5M2 by default): add, sub, mul, div.
 * Portable C for the targets that have no hand-written version (config/6502/sf8.s is the 6502 one). Same algorithm:
 * operands normalized on unpack, a left-aligned 8-bit significand with lost bits jammed into bit 0, one packer.
 * SF8_NAME(n) is `__fp8_n` in the runtime, `sf8b_n` in the host test. Int is 16 bits wide enough. */
#include <stdint.h>

#ifndef SF8_NAME
#define SF8_CAT(a, b) a##b
#define SF8_NAME(n) SF8_CAT(__fp8_, n)
#endif
#ifndef FP8_M
#define FP8_M 2
#endif
#define M FP8_M
#define E (7 - M)
#define BIAS ((1 << (E - 1)) - 1)
#define EMAX ((1 << E) - 1)
#define INF (EMAX << M)
#define QNAN (INF | (1 << (M - 1)))
#define HID (1 << M)
#define SIGSH (7 - M)
#define GUARD (1 << (SIGSH - 1))
#define REST (GUARD - 1)

typedef uint8_t fp8;

static void unpack(fp8 v, uint8_t *s, int8_t *e, uint8_t *m)
{
    *s = v & 0x80;
    *e = (v >> M) & EMAX;
    *m = v & (HID - 1);
    if (*e) *m |= HID;
    else {
        *e = 1;
        while (*m && *m < HID) { *m <<= 1; (*e)--; }
    }
}

/* e: exponent of the leading one (may be <= 0), pm: significand with its one in bit 7, lost bits jammed */
static fp8 pack(uint8_t s, int e, uint8_t pm)
{
    if (e <= 0) {
        int n = 1 - e;
        if (n > 8) n = 8;
        while (n--) pm = (pm >> 1) | (pm & 1);
        e = 1;
    }
    if (e >= EMAX) return s | INF;
    uint8_t code = (uint8_t)(((e - 1) << M) + (pm >> SIGSH));
    if ((pm & GUARD) && ((pm & REST) || (code & 1))) code++;
    return s | (code > INF ? INF : code);
}

static int is_nan(fp8 v) { return (v & 0x7F) > INF; }
static int is_inf(fp8 v) { return (v & 0x7F) == INF; }

fp8 SF8_NAME(add)(fp8 a, fp8 b)
{
    if (is_nan(a) || is_nan(b)) return QNAN;
    if (is_inf(a) || is_inf(b)) {
        if (is_inf(a) && is_inf(b) && ((a ^ b) & 0x80)) return QNAN;
        return is_inf(a) ? a : b;
    }
    if ((b & 0x7F) > (a & 0x7F)) { fp8 t = a; a = b; b = t; }
    uint8_t sa, ma, sb, mb;
    int8_t ea, eb;
    unpack(a, &sa, &ea, &ma);
    unpack(b, &sb, &eb, &mb);
    uint8_t pm = (uint8_t)(ma << SIGSH), bm = (uint8_t)(mb << SIGSH);
    int n = ea - eb, e = ea;
    if (n > 8) n = 8;
    while (n--) bm = (bm >> 1) | (bm & 1);
    if (sa == sb) {
        unsigned sum = pm + bm;
        pm = (uint8_t)sum;
        if (sum > 255) { pm = (uint8_t)((sum >> 1) | (sum & 1) | 0x80); e++; }
    } else {
        pm -= bm;
        if (!pm) return 0;
        while (!(pm & 0x80) && e > 1) { pm <<= 1; e--; }
    }
    return pack(sa, e, pm);
}

fp8 SF8_NAME(sub)(fp8 a, fp8 b) { return SF8_NAME(add)(a, b ^ 0x80); }

fp8 SF8_NAME(mul)(fp8 a, fp8 b)
{
    uint8_t s = (a ^ b) & 0x80;
    if (is_nan(a) || is_nan(b)) return QNAN;
    if (is_inf(a) || is_inf(b)) return (!(a & 0x7F) || !(b & 0x7F)) ? QNAN : s | INF;
    uint8_t sa, ma, sb, mb;
    int8_t ea, eb;
    unpack(a, &sa, &ea, &ma);
    unpack(b, &sb, &eb, &mb);
    if (!ma || !mb) return s;
    unsigned p = (unsigned)ma * mb;                 /* leading one at bit 2M or 2M+1 */
    int e = ea + eb - BIAS, l = 2 * M;
    uint8_t pm;
    if (p >> (2 * M + 1)) { l++; e++; }
    if (l <= 7) pm = (uint8_t)(p << (7 - l));
    else {                                          /* M = 4: bring it down to bit 7, jamming what falls off */
        unsigned n = l - 7;
        pm = (uint8_t)(p >> n);
        if (p & ((1u << n) - 1)) pm |= 1;
    }
    return pack(s, e, pm);
}

fp8 SF8_NAME(div)(fp8 a, fp8 b)
{
    uint8_t s = (a ^ b) & 0x80;
    if (is_nan(a) || is_nan(b)) return QNAN;
    if (is_inf(a)) return is_inf(b) ? QNAN : s | INF;
    if (is_inf(b)) return s;
    uint8_t sa, ma, sb, mb;
    int8_t ea, eb;
    unpack(a, &sa, &ea, &ma);
    unpack(b, &sb, &eb, &mb);
    if (!mb) return ma ? s | INF : QNAN;
    if (!ma) return s;
    unsigned r = ma, q = 0;
    for (int i = 0; i < M + 3; i++) {
        q <<= 1;
        if (r >= mb) { r -= mb; q |= 1; }
        r <<= 1;
    }
    int e = ea - eb + BIAS - 1;
    if (q >= (1u << (M + 2))) { q <<= 5 - M; e++; } else q <<= 6 - M;
    return pack(s, e, (uint8_t)(q | (r != 0)));
}
