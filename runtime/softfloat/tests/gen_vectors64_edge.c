/* Like gen_vectors64.c (same vectors64.h layout), but the operands target the hand-written m68k paths (src/m68k/sf64.s)
 * that random bits rarely reach. A model of the divide counts its rare steps; a run of 2000 or more fails if one is never hit.
 * cc -O2 -ffp-contract=off tests/gen_vectors64_edge.c -o gen && ./gen [n] > vectors64.h */
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

/* The divide: U = ma << 11 (halved when ma >= mb), V = mb << 11, quotient digits of w bits (16: 68000/68060, 32: 68020).
 * qhat comes from the remainder's top 2w bits over V's top w bits; ovf[w/16-1][k] counts digit k with equal top halves
 * (qhat forced to all ones), fix[w/16-1][c] digits whose qhat was c too big. */
static long ovf[2][4], fix[2][3];
static void div_model(uint64_t ma, uint64_t mb, int w)
{
    uint64_t U = ma << 11, V = mb << 11, vt = V >> (64 - w), R = ma >= mb ? U >> 1 : U, mask = (1ull << w) - 1;
    for (int k = 0; k < 64 / w; k++) {
        uint64_t rt = R >> (64 - w), qhat = rt == vt ? mask : (uint64_t)((R >> (64 - 2 * w)) / vt);
        if (rt == vt) ovf[w / 16 - 1][k]++;
        u128 n = (u128)R << w;
        uint64_t q = (uint64_t)(n / V);
        fix[w / 16 - 1][qhat - q > 2 ? 2 : qhat - q]++;
        R = (uint64_t)(n % V);
    }
}

/* ma, mb whose remainder before digit k (0-based, w-bit digits) has V's top w bits: U * 2^(wk) = Q * V + R with R's top w bits
 * those of V and U's low 11 bits zero. Q is drawn until R's forced low bits and top bits agree. */
static void div_digit_overflow(int w, int k, uint64_t *ma, uint64_t *mb)
{
    for (;;) {
        uint64_t V = (HID | (rnd() & (HID - 1))) << 11, top = V >> (64 - w) << (64 - w);
        if (k == 0) { /* U < V with the same top bits */
            uint64_t U = top | ((rnd() % (V - top)) & ~0x7FFull);
            *ma = U >> 11, *mb = V >> 11; return;
        }
        int L = w * k + 11;
        u128 Q = (u128)1 << (w * k - 1) | (((u128)rnd() << 64 | rnd()) & (((u128)1 << (w * k - 1)) - 1));
        uint64_t topmask = ~0ull << (64 - w), lowmask = (1ull << L) - 1, low = (uint64_t)(-(Q * V)) & lowmask;
        if ((low & topmask) != (top & lowmask)) continue;
        uint64_t R = top | (low & ~topmask) | (rnd() & ~lowmask & ~topmask);
        if (R >= V) continue;
        u128 n = Q * V + R;
        if ((uint64_t)n & 0x7FF || (n >> (w * k)) >> 64) continue;
        uint64_t U = (uint64_t)(n >> (w * k));
        if (U < V >> 1 || U >= V) continue;
        *ma = U >> 11, *mb = V >> 11; return;
    }
}

static void pair(uint64_t *a, uint64_t *b)
{
    uint64_t s1 = rnd() & 1, s2 = rnd() & 1, e = 1 + rnd() % 2046, m = rnd(), m2 = rnd(), ma, mb;
    switch (rnd() % 12) {
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
        int w = rnd() & 1 ? 32 : 16, k = rnd() % (64 / w);
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
int main(int argc, char **argv)
{
    int n = argc > 1 ? atoi(argv[1]) : 3000;
    printf("/* generated by tests/gen_vectors64_edge.c */\n#define NV %d\n", n);
    printf("static const uint32_t V[NV][16] = { /* a, b, add, sub, mul, div (hi,lo each), cmp, floatsidf(a.lo) (hi,lo), fixdfsi(a) */\n");
    for (int i = 0; i < n; i++) {
        uint64_t a, b;
        pair(&a, &b);
        uint64_t ea = a >> 52 & 0x7FF, eb = b >> 52 & 0x7FF;
        if (ea - 1 < 2046 && eb - 1 < 2046) { div_model(HID | (a & (HID - 1)), HID | (b & (HID - 1)), 16); div_model(HID | (a & (HID - 1)), HID | (b & (HID - 1)), 32); }
        volatile double x = b2d(a), y = b2d(b);
        int cmp = x < y ? -1 : x > y ? 1 : x == y ? 0 : 2;
        volatile double fi = (double)(int32_t)(uint32_t)a;
        int ok = x > -2147483649.0 && x < 2147483648.0;
        uint64_t r[6] = {a, b, d2b(x + y), d2b(x - y), d2b(x * y), d2b(x / y)};
        printf("  {");
        for (int k = 0; k < 6; k++) printf("0x%08Xu,0x%08Xu,", W(r[k]));
        printf("%d,0x%08Xu,0x%08Xu,0x%08Xu},\n", cmp, W(d2b(fi)), ok ? (uint32_t)(int32_t)x : 0xDEADBEEFu);
    }
    printf("};\n");
    int miss = 0;
    for (int w = 0; w < 2; w++) {
        for (int k = 0; k < 4 / (w + 1); k++) miss |= !ovf[w][k];
        miss |= !fix[w][1] || !fix[w][2];
    }
    if (n >= 2000 && miss) {
        fprintf(stderr, "gen_vectors64_edge: a divide step never hit: ovf16 %ld %ld %ld %ld ovf32 %ld %ld fix16 %ld %ld fix32 %ld %ld\n", ovf[0][0],
                ovf[0][1], ovf[0][2], ovf[0][3], ovf[1][0], ovf[1][1], fix[0][1], fix[0][2], fix[1][1], fix[1][2]);
        return 1;
    }
    return 0;
}
