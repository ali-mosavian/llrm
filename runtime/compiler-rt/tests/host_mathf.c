/* f32 libm subset vs the host's double libm rounded to float, in ulps (sqrtf must be exact).
 * cc -O2 -ffp-contract=off -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' '-DSFM_NAME(n)=sfb_##n' \
 *    tests/host_mathf.c lib/math/mathf.c lib/fp/sf32_conv.c lib/fp/sf32.c lib/fp/sf32_conv64.c -lm */
#define SF_NAME(n) sfb_##n
#include "softfloat.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

float sfb_sqrtf(float), sfb_sinf(float), sfb_cosf(float), sfb_tanf(float), sfb_atanf(float), sfb_asinf(float),
    sfb_acosf(float), sfb_expf(float), sfb_exp2f(float), sfb_logf(float), sfb_log2f(float), sfb_log10f(float),
    sfb_atan2f(float, float);

static uint64_t rng = 0x9E3779B97F4A7C15ull;
static uint32_t rnd(void) { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; return (uint32_t)(rng >> 16); }

static long ulps(float a, float b)
{
    sf32 x = f2b(a), y = f2b(b);
    if ((x << 1) > 0xFF000000u || (y << 1) > 0xFF000000u) return ((x << 1) > 0xFF000000u) == ((y << 1) > 0xFF000000u) ? 0 : 1 << 30;
    long ix = (x & 0x80000000u) ? -(long)(x & 0x7FFFFFFFu) : (long)x;
    long iy = (y & 0x80000000u) ? -(long)(y & 0x7FFFFFFFu) : (long)y;
    return labs(ix - iy);
}

struct fn { const char *name; float (*f)(float); double (*ref)(double); long lim, max, n; sf32 worst; };
static double r_sq(double x) { return sqrt(x); }

static void run(struct fn *t, sf32 b)
{
    float x = b2f(b);
    volatile float got = t->f(x);
    float want = (float)t->ref((double)x);
    long u = ulps(got, want);
    t->n++;
    if (u > t->max) { t->max = u; t->worst = b; }
}

int main(int argc, char **argv)
{
    long stride = argc > 1 ? atol(argv[1]) : 997;
    struct fn t[] = {
        {"sqrtf", sfb_sqrtf, r_sq, 0}, {"sinf", sfb_sinf, sin, 3}, {"cosf", sfb_cosf, cos, 3}, {"tanf", sfb_tanf, tan, 4},
        {"atanf", sfb_atanf, atan, 2}, {"asinf", sfb_asinf, asin, 3}, {"acosf", sfb_acosf, acos, 3},
        {"expf", sfb_expf, exp, 2}, {"exp2f", sfb_exp2f, exp2, 2}, {"logf", sfb_logf, log, 2}, {"log2f", sfb_log2f, log2, 2},
        {"log10f", sfb_log10f, log10, 3},
    };
    int nt = sizeof t / sizeof *t, bad = 0;
    /* Regression: reduce_large ignored the sign of x, so sinf(-1.5709091f) returned +1 (and tanf the wrong sign)
     * for every negative x within 2^-12 of a multiple of pi/2. */
    static const sf32 neg_near_half_pi[] = {0xBFC9138Du, 0xC969C515u, 0xBFC90FDBu, 0xC0490FDBu};
    for (unsigned i = 0; i < sizeof neg_near_half_pi / sizeof *neg_near_half_pi; i++)
        for (int j = 0; j < nt; j++) run(&t[j], neg_near_half_pi[i]);
    for (uint64_t b = 0; b < 0x100000000ull; b += stride)
        for (int i = 0; i < nt; i++) run(&t[i], (sf32)b);
    for (long i = 0; i < 2000000; i++) {          /* uniform in magnitude near 1 and in [-100, 100] */
        float x = (float)((double)rnd() / 4294967296.0 * 200.0 - 100.0);
        for (int j = 0; j < nt; j++) run(&t[j], f2b(x));
        x = (float)((double)rnd() / 4294967296.0 * 2.0 - 1.0);
        for (int j = 0; j < nt; j++) run(&t[j], f2b(x));
    }
    for (int i = 0; i < nt; i++) {
        printf("%-7s n=%ld max=%ld ulp (limit %ld) worst=%08x\n", t[i].name, t[i].n, t[i].max, t[i].lim, t[i].worst);
        bad |= t[i].max > t[i].lim;
    }
    long m2 = 0, n2 = 0;
    for (long i = 0; i < 3000000; i++) {
        float y = b2f(rnd()), x = b2f(rnd());
        if (i & 1) { y = (float)((int)(rnd() % 2001) - 1000) / 7.0f; x = (float)((int)(rnd() % 2001) - 1000) / 5.0f; }
        long u = ulps(sfb_atan2f(y, x), (float)atan2((double)y, (double)x));
        n2++;
        if (u > m2) { m2 = u; if (u > 3) printf("atan2 y=%08x x=%08x %ld\n", f2b(y), f2b(x), u); }
    }
    printf("atan2f  n=%ld max=%ld ulp (limit 3)\n", n2, m2);
    bad |= m2 > 3;
    return bad;
}
