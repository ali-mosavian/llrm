/* Rankin/Wozniak 6502 floating point (Dr. Dobb's Journal, August 1976, public domain), translated routine by routine
 * into C: ADD, FCOMPL, NORM, RTLOG/RTAR, SWAP, ABSWAP/MD1/MD2, FADD/FSUB, FMUL, FDIV, FLOAT, FIX. Register state is the
 * zero-page workspace of the original (X1/M1 and X2/M2 accumulators, E, SIGN) plus the carry and overflow flags the
 * routines hand to each other. tests/woz_compare/xcheck.py runs the original machine code and this file on the same
 * operands and requires identical bytes.
 *
 * Word layout: (M << 8) | X, X the exponent byte (biased, value = M / 2^22 * 2^(X - 128)), M a 24-bit two's
 * complement mantissa. Truncating; no denormals, inf or NaN. Where the original jumps to its overflow vector, these
 * functions return a saturated value instead. */
#include <stdint.h>

#ifndef FZW_NAME
#define FZW_NAME(n) fzw_##n
#endif

typedef uint32_t fz;
#define MASK 0xFFFFFFu

typedef struct {
    uint32_t m1, m2, e;
    uint8_t x1, x2, sign, a;       /* a: the accumulator after the last SWAP */
    uint8_t c, v, ovf;             /* carry, overflow flag, "jumped to the overflow vector" */
} fp;

static void add(fp *s)             /* ADD: M1 += M2, 24 bits */
{
    uint32_t a = s->m1, b = s->m2, r = a + b;
    s->c = (uint8_t)(r >> 24);
    s->m1 = r & MASK;
    s->v = (uint8_t)((~(a ^ b) & (a ^ s->m1)) >> 23) & 1;
}

static void norm1(fp *s)           /* NORM1: shift left until the top two mantissa bits differ or the exponent is 0 */
{
    for (;;) {
        uint8_t hi = (uint8_t)(s->m1 >> 16);
        s->c = hi >= 0xC0;
        if ((uint8_t)(hi - 0xC0) & 0x80) return;
        s->x1--;
        s->c = (uint8_t)(s->m1 >> 23);
        s->m1 = (s->m1 << 1) & MASK;
        if (!s->x1) return;
    }
}

static void norm(fp *s) { if (s->x1) norm1(s); }

static void rtlog1(fp *s)          /* RTLOG1: M1:E right one bit through the carry */
{
    uint8_t out = (uint8_t)(s->e & 1);
    s->e = (s->e >> 1) | ((s->m1 & 1) << 23);
    s->m1 = (s->m1 >> 1) | ((uint32_t)s->c << 23);
    s->c = out;
}

static void rtlog(fp *s)           /* RTLOG: count the shift in X1, then shift */
{
    if (!++s->x1) { s->ovf = 1; return; }
    rtlog1(s);
}

static void rtar(fp *s) { s->c = (uint8_t)(s->m1 >> 23); rtlog(s); }

static void swap(fp *s)            /* SWAP: exchange X1/M1 with X2/M2, leave the new M1 in E */
{
    uint8_t x = s->x1; uint32_t m = s->m1;
    s->a = x;
    s->x1 = s->x2; s->m1 = s->m2;
    s->x2 = x; s->m2 = m;
    s->e = s->m1;
}

static void fcompl(fp *s)          /* FCOMPL: M1 = -M1, then normalize or, on overflow, shift right */
{
    uint32_t m = s->m1;
    s->c = m == 0;
    s->v = m == 0x800000u;
    s->m1 = (0u - m) & MASK;
    if (s->v) rtlog(s); else norm(s);
}

static void abswap(fp *s)
{
    if (s->m1 >> 23) { fcompl(s); if (s->ovf) return; s->sign++; }
    s->c = 1;
    swap(s);
}

static void md1(fp *s) { s->sign <<= 1; abswap(s); if (!s->ovf) abswap(s); }

/* MD2: A is the combined exponent, carry from computing it. Returns 1 to continue, 0 when the caller must stop
 * (overflow, or underflow where the result is cleared to zero by NORM). */
static int md2(fp *s, uint8_t a, uint8_t c)
{
    s->m1 = 0;
    if (c ? (a & 0x80) : !(a & 0x80)) {
        if (c) { s->ovf = 1; return 0; }
        norm(s);                   /* underflow: M1 is 0, so NORM counts X1 down to 0 */
        return 0;
    }
    s->x1 = a ^ 0x80;
    return 1;
}

static void normx(fp *s)           /* MDEND: negate when exactly one operand was negative */
{
    uint8_t c = s->sign & 1;
    s->sign >>= 1;
    if (!c) norm(s); else fcompl(s);
}

static void fadd(fp *s)
{
    for (;;) {
        if (s->x2 == s->x1) break;
        s->c = s->x2 >= s->x1;     /* CMP X1 */
        if (!s->c) swap(s); else { rtar(s); if (s->ovf) return; }
    }
    add(s);
    if (!s->v) norm(s); else rtlog(s);
}

static void fsub(fp *s)            /* FP2 - FP1 */
{
    fcompl(s);
    if (s->ovf) return;
    if (!s->c) swap(s); else { rtar(s); if (s->ovf) return; }
    fadd(s);
}

static void fmul(fp *s)
{
    md1(s);
    if (s->ovf) return;
    unsigned sum = (unsigned)s->a + s->x1 + s->c;
    if (!md2(s, (uint8_t)sum, (uint8_t)(sum >> 8))) return;
    s->c = 0;
    for (int y = 23; y >= 0; y--) {
        rtlog1(s);
        if (s->c) add(s);
    }
    normx(s);
}

static void fdiv(fp *s)
{
    md1(s);
    if (s->ovf) return;
    uint8_t a = (uint8_t)(s->a - s->x1), c = s->a >= s->x1;
    if (!md2(s, a, c)) return;
    for (int y = 23; y > 0; y--) {
        uint8_t q = s->m2 >= s->e;
        if (q) s->m2 = (s->m2 - s->e) & MASK;
        s->m1 = ((s->m1 << 1) | q) & MASK;
        uint8_t out = (uint8_t)(s->m2 >> 23);
        s->m2 = (s->m2 << 1) & MASK;
        if (out) { s->ovf = 1; return; }
    }
    normx(s);
}

static fz out(fp *s) { return s->ovf ? 0x7FFFFFFFu : (s->m1 << 8) | s->x1; }
static fp load(fz a, fz b)          /* FP1 = a, FP2 = b */
{
    fp s = {0};
    s.x1 = (uint8_t)a; s.m1 = a >> 8; s.x2 = (uint8_t)b; s.m2 = b >> 8;
    return s;
}

fz FZW_NAME(add)(fz a, fz b) { fp s = load(a, b); fadd(&s); return out(&s); }
fz FZW_NAME(sub)(fz a, fz b) { fp s = load(b, a); fsub(&s); return out(&s); }   /* a - b */
fz FZW_NAME(mul)(fz a, fz b) { fp s = load(a, b); fmul(&s); return out(&s); }
fz FZW_NAME(div)(fz a, fz b) { fp s = load(b, a); fdiv(&s); return out(&s); }   /* a / b */

fz FZW_NAME(from_i16)(int16_t n)
{
    fp s = load(0, 0);
    s.m1 = ((uint32_t)(uint16_t)n << 8) & MASK;
    s.x1 = 0x8E;
    norm1(&s);
    return out(&s);
}

int16_t FZW_NAME(to_i16)(fz a)
{
    fp s = load(a, 0);
    for (;;) {
        if (!(s.x1 & 0x80)) return 0;                 /* UNDFL */
        if (s.x1 == 0x8E) break;
        rtar(&s);
        if (s.ovf) return (int16_t)(s.m1 >> 23 ? -32768 : 32767);
    }
    uint32_t m = s.m1;
    uint16_t r = (uint16_t)(m >> 8);
    if ((m >> 23) && (m & 0xFF)) r++;                 /* negative with a fraction: toward zero */
    return (int16_t)r;
}
