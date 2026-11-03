/* Like gen_vectors64.c (same vectors64.h layout), but the operands target the hand-written m68k paths (config/m68k/sf64.s)
 * that random bits rarely reach. A model of the divide counts its rare steps; a run of 2000 or more fails if one is never hit.
 * cc -O2 -ffp-contract=off tests/gen_vectors64_edge.c -o gen && ./gen [n [start [count]]] > vectors64.h
 * The stream is the first n pairs (the divide-step check runs over all n); records start..start+count-1 are written. -DEXT: see put_record. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef unsigned __int128 u128;
static uint64_t rng = 0xD1B54A32D192ED03ull;
static uint64_t rnd(void) { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; return rng * 0x2545F4914F6CDD1Dull; }
static double b2d(uint64_t b) { double d; memcpy(&d, &b, 8); return d; }
static uint64_t d2b(double d) { uint64_t b; memcpy(&b, &d, 8); return b; }
static uint64_t mk(uint64_t s, uint64_t e, uint64_t m) { return s << 63 | (e & 0x7FF) << 52 | (m & 0xFFFFFFFFFFFFFull); }
#define HID (1ull << 52)

/* The divide: U = ma << 3, V = mb << 11, quotient digits of w bits (16: 68000/68060, 32: 68020).
 * qhat comes from the remainder's top 2w bits over V's top w bits; ovf[w/16-1][k] counts digit k with equal top halves
 * (qhat forced to all ones), fix[w/16-1][c] digits whose qhat was c too big. */
static long ovf[2][4], fix[2][3];
static void div_model(uint64_t ma, uint64_t mb, int w)
{
    uint64_t V = mb << 11, vt = V >> (64 - w), R = ma << 3, mask = (1ull << w) - 1;
    for (int k = 0; k < 64 / w; k++) {
        uint64_t rt = R >> (64 - w), qhat = rt == vt ? mask : (uint64_t)((R >> (64 - 2 * w)) / vt);
        if (rt == vt) ovf[w / 16 - 1][k]++;
        u128 n = (u128)R << w;
        uint64_t q = (uint64_t)(n / V);
        fix[w / 16 - 1][qhat - q > 2 ? 2 : qhat - q]++;
        R = (uint64_t)(n % V);
    }
}

/* ma, mb whose remainder before digit k (1-based: U < 2^56 cannot tie V's top bits) has V's top w bits:
 * U * 2^(wk) = Q * V + R with R's top w bits those of V and U's low 3 bits zero. Q is drawn until R's forced low bits and top bits agree. */
static void div_digit_overflow(int w, int k, uint64_t *ma, uint64_t *mb)
{
    for (;;) {
        uint64_t V = (HID | (rnd() & (HID - 1))) << 11, top = V >> (64 - w) << (64 - w);
        int L = w * k + 3;
        u128 lo = ((u128)1 << (w * k + 55)) / V, span = lo, r = (u128)rnd() << 64 | rnd();
        u128 Q = lo + r % span;
        uint64_t topmask = ~0ull << (64 - w), lowmask = (1ull << L) - 1, low = (uint64_t)(-(Q * V)) & lowmask;
        if ((low & topmask) != (top & lowmask)) continue;
        uint64_t R = top | (low & ~topmask) | (rnd() & ~lowmask & ~topmask);
        if (R >= V) continue;
        u128 n = Q * V + R;
        if ((uint64_t)n & 7 || (n >> (w * k)) >> 64) continue;
        uint64_t U = (uint64_t)(n >> (w * k));
        if (U >> 55 != 1) continue;
        *ma = U >> 3, *mb = V >> 11; return;
    }
}

static void pair(uint64_t *a, uint64_t *b)
{
    uint64_t s1 = rnd() & 1, s2 = rnd() & 1, e = 1 + rnd() % 2046, m = rnd(), m2 = rnd(), ma, mb;
    static const int gaps[] = {0, 1, 2, 3, 31, 32, 33, 52, 53, 54, 55, 56};
    switch (rnd() % 17) {
    case 12: /* add/sub at exponent gaps on the word boundaries and the far-gap return, patterns at the rounding edge */
        { int g = gaps[rnd() % 12]; uint64_t ea = 60 + rnd() % 1980, p = rnd() % 4;
          *a = mk(s1, ea, p == 0 ? 0 : p == 1 ? ~0ull : m), *b = mk(s2, ea - g, p == 2 ? 0 : p == 3 ? ~0ull : m2);
          if (rnd() & 1) { uint64_t t = *a; *a = *b; *b = t; } return; }
    case 13: /* cancellation: equal or adjacent exponents, mantissas close in the high or low word */
        *a = mk(s1, e, m), *b = mk(s1 ^ 1, e - (rnd() & 1), rnd() & 1 ? m + (rnd() % 7) - 3 : (m & ~0xFFFFFFFFull) | (m2 & 0xFFFFFFFFull)); return;
    case 14: /* same-sign add near the top: carries into inf */
        *a = mk(s1, 2046 - rnd() % 2, m | (rnd() & 1 ? 0xFFFFFFFFFFFFFull : 0)), *b = mk(s1, 2040 + rnd() % 7, m2); return;
    case 15: /* add ties: b is half an ulp of a times an odd or even pattern */
        *a = mk(s1, e, m), *b = mk(s2, e > 53 ? e - 53 : 0, rnd() & 1 ? 0 : m2 & 1); return;
    case 16: /* denormals and the smallest normals against each other, x - x */
        *a = mk(s1, rnd() % 3, m), *b = rnd() % 4 ? mk(s2, rnd() % 3, m2) : *a ^ ((rnd() & 1) << 63); return;
    case 8: /* mul ties: 27-bit mantissas make 54-bit products */
        *a = mk(s1, e, m & 0xFFFFFFC000000ull), *b = mk(s2, 1 + rnd() % 2046, m2 & 0xFFFFFFC000000ull); return;
    case 9: /* mul products just under a power of two: the rounding carry moves the exponent */
        ma = HID | (m & (HID - 1)), mb = (uint64_t)(((u128)1 << 105) / ma) + (rnd() % 3) - 1;
        if (mb >= HID << 1) mb = (HID << 1) - 1;
        *a = mk(s1, e, ma), *b = mk(s2, 1 + rnd() % 2046, mb); return;
    case 10: /* mul results in or near the denormal range and at the overflow edge */
        if (rnd() & 1) { uint64_t ea = 1 + rnd() % 1000; *a = mk(s1, ea, m), *b = mk(s2, 1023 - 60 - ea + rnd() % 64, rnd() & 1 ? m2 : 0); }
        else { uint64_t ea = 1024 + rnd() % 1000; *a = mk(s1, ea, m), *b = mk(s2, 3069 - ea - rnd() % 3, rnd() & 1 ? m2 : 0xFFFFFFFFFFFFFull); }
        return;
    case 11: /* denormal times a large normal: results anywhere around the bottom of the normal range */
        *a = mk(s1, 0, m >> (rnd() % 52)), *b = mk(s2, 1000 + rnd() % 100, m2); if (rnd() & 1) { uint64_t t = *a; *a = *b; *b = t; } return;
    case 0: case 1: { /* divide digit k with equal top halves, 16- and 32-bit digits */
        int w = rnd() & 1 ? 32 : 16, k = 1 + rnd() % (64 / w - 1);
        div_digit_overflow(w, k, &ma, &mb);
        uint64_t ea = 1 + rnd() % 2046, eb = 1 + rnd() % 2046;
        *a = mk(s1, ea, ma), *b = mk(s2, eb, mb); return;
    }
    case 2: /* quotients in or near the denormal range, ties included (small mantissas divide exactly) */
        *a = mk(s1, rnd() % 60, rnd() & 1 ? m : m & ~0xFFFFFFFull), *b = mk(s2, 1000 + rnd() % 80, rnd() & 1 ? 0 : m2 & 0xF000000000000ull); return;
    case 3: /* near the overflow edge */
        *a = mk(s1, 2046 - rnd() % 3, rnd() & 1 ? ~0ull : m), *b = mk(s2, 1022 + rnd() % 3, rnd() & 1 ? 0 : m2); return;
    case 4: /* denormal operands of every length against normals and denormals */
        *a = mk(s1, 0, (1ull << (rnd() % 52)) | (rnd() & 1 ? m >> (12 + rnd() % 52) : 0)), *b = rnd() & 1 ? mk(s2, e, m2) : mk(s2, 0, m2 >> (rnd() % 52));
        if (rnd() & 1) { uint64_t t = *a; *a = *b; *b = t; } return;
    case 5: /* exact quotients: a = b * small integer */
        { double q = (double)(int)(rnd() % 4096) + 1, y = b2d(mk(s2, 900 + rnd() % 200, m2 & ~0xFFFFull)); *a = d2b(y * q); *b = d2b(y); return; }
    case 6: /* mantissas equal or adjacent */
        *a = mk(s1, e, m), *b = mk(s2, 1 + rnd() % 2046, m + (rnd() % 5) - 2); return;
    default: /* inf, NaN, zero against anything */
        { static const uint64_t sp[] = {0x7FF0000000000000ull, 0xFFF0000000000000ull, 0x7FF8000000000000ull, 0x7FF0000000000001ull,
                                        0xFFFFFFFFFFFFFFFFull, 0, 0x8000000000000000ull, 1, 0x7FF4000000000000ull};
          *a = sp[rnd() % 9], *b = rnd() % 3 ? sp[rnd() % 9] : mk(s2, e, m2); if (rnd() & 1) { uint64_t t = *a; *a = *b; *b = t; } }
    }
}

#define W(v) (unsigned)((v) >> 32), (unsigned)(v)

/* -DEXT appends the conversions for the 6502 harness (sim65_main64.c): words 16.. = floatunsidf(a.lo), floatdidf(a), floatundidf(a)
 * (hi,lo each), then fixdfsi, fixunsdfsi, fixdfdi, fixunsdfdi of a, saturating, NaN as 0 (fixdfdi, fixunsdfdi: hi,lo),
 * then extendsfdf2(a.lo as a float) (hi,lo) and truncdfsf2(a). */
#ifdef EXT
#define NW 31
static int32_t sat_i32(double x) { return x != x ? 0 : x >= 2147483648.0 ? INT32_MAX : x <= -2147483649.0 ? INT32_MIN : (int32_t)x; }
static uint32_t sat_u32(double x) { return x != x ? 0 : x >= 4294967296.0 ? UINT32_MAX : x <= -1.0 ? 0 : (uint32_t)x; }
static int64_t sat_i64(double x) { return x != x ? 0 : x >= 9223372036854775808.0 ? INT64_MAX : x <= -9223372036854775808.0 ? INT64_MIN : (int64_t)x; }
static uint64_t sat_u64(double x) { return x != x ? 0 : x >= 18446744073709551616.0 ? UINT64_MAX : x <= -1.0 ? 0 : (uint64_t)x; }
#else
#define NW 16
#endif
static void put_record(uint64_t a, uint64_t b)
{
    volatile double x = b2d(a), y = b2d(b);
    int cmp = x < y ? -1 : x > y ? 1 : x == y ? 0 : 2;
    volatile double fi = (double)(int32_t)(uint32_t)a;
    int ok = x > -2147483649.0 && x < 2147483648.0;
    uint64_t r[6] = {a, b, d2b(x + y), d2b(x - y), d2b(x * y), d2b(x / y)};
    printf("  {");
    for (int k = 0; k < 6; k++) printf("0x%08Xu,0x%08Xu,", W(r[k]));
    printf("%d,0x%08Xu,0x%08Xu,0x%08Xu", cmp, W(d2b(fi)), ok ? (uint32_t)(int32_t)x : 0xDEADBEEFu);
#ifdef EXT
    volatile double fu = (double)(uint32_t)a, fd = (double)(int64_t)a, fud = (double)a;
    printf(",0x%08Xu,0x%08Xu,0x%08Xu,0x%08Xu,0x%08Xu,0x%08Xu", W(d2b(fu)), W(d2b(fd)), W(d2b(fud)));
    printf(",0x%08Xu,0x%08Xu,0x%08Xu,0x%08Xu,0x%08Xu,0x%08Xu", (unsigned)sat_i32(x), sat_u32(x), W((uint64_t)sat_i64(x)), W(sat_u64(x)));
    uint32_t lo = (uint32_t)a;
    volatile float fl; memcpy((void *)&fl, &lo, 4);
    volatile double fe = fl;
    volatile float tr = (float)x;
    uint32_t trb; memcpy(&trb, (const void *)&tr, 4);
    printf(",0x%08Xu,0x%08Xu,0x%08Xu", W(d2b(fe)), trb);
#endif
    printf("},\n");
}

int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 3000, start = argc > 2 ? atoi(argv[2]) : 0, count = argc > 3 ? atoi(argv[3]) : n - start;
    printf("/* generated by tests/gen_vectors64_edge.c */\n#define NV %d\n", count);
    printf("static const uint32_t V[NV][%d] = { /* a, b, add, sub, mul, div (hi,lo each), cmp, floatsidf(a.lo) (hi,lo), fixdfsi(a), [EXT: see put_record] */\n", NW);
    for (int i = 0; i < n; i++) {
        uint64_t a, b;
        pair(&a, &b);
        uint64_t ea = a >> 52 & 0x7FF, eb = b >> 52 & 0x7FF;
        if (ea - 1 < 2046 && eb - 1 < 2046) { div_model(HID | (a & (HID - 1)), HID | (b & (HID - 1)), 16); div_model(HID | (a & (HID - 1)), HID | (b & (HID - 1)), 32); }
        if (i >= start && i < start + count) put_record(a, b);
    }
    printf("};\n");
    int miss = 0;
    for (int w = 0; w < 2; w++) {
        for (int k = 1; k < 4 / (w + 1); k++) miss |= !ovf[w][k];
        miss |= !fix[w][1] || !fix[w][2];
    }
    if (n >= 2000 && miss) {
        fprintf(stderr, "gen_vectors64_edge: a divide step never hit: ovf16 %ld %ld %ld ovf32 %ld fix16 %ld %ld fix32 %ld %ld\n", ovf[0][1],
                ovf[0][2], ovf[0][3], ovf[1][1], fix[0][1], fix[0][2], fix[1][1], fix[1][2]);
        return 1;
    }
    return 0;
}
