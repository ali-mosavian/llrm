/* fast32 spike vs double arithmetic: truncating, so error <= 2^-21 of the larger of operands and result.
 * cc -O2 -Iinclude tests/host_fast32.c lib/fast/fast32.c -lm */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
typedef uint32_t fz;
fz fz_add(fz, fz), fz_sub(fz, fz), fz_mul(fz, fz), fz_div(fz, fz), fz_neg(fz), fz_from_i16(int16_t);
int16_t fz_to_i16(fz);
static uint64_t rng = 0x9E3779B97F4A7C15ull;
static uint32_t rnd(void) { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; return (uint32_t)(rng >> 16); }
static double dec(fz w) { int32_t m = (int32_t)w >> 8; return ldexp((double)m / 4194304.0, (int)(w & 255) - 128); }
static fz gen(void)
{
    uint32_t r = rnd();
    int32_t m;
    do m = (int32_t)(rnd() & 0xFFFFFF) - 0x800000; while (m == 0 || !(((m >> 23) ^ (m >> 22)) & 1) || 0);
    if (r & 1) { m = (r & 2) ? 0x400000 : -0x800000; }                      /* extreme mantissas */
    return ((uint32_t)m << 8) | (uint32_t)(100 + (rnd() % 56));
}
static long fails, n[6];
static void chk(int op, const char *nm, fz a, fz b, double got, double want, double scale)
{
    n[op]++;
    if (fabs(got - want) > scale * ldexp(1, -21) && fails++ < 20) printf("FAIL %s a=%08x b=%08x got=%g want=%g\n", nm, a, b, got, want);
}
int main(int argc, char **argv)
{
    long cnt = argc > 1 ? atol(argv[1]) : 3000000;
    for (long i = 0; i < cnt; i++) {
        fz a = gen(), b = gen();
        double x = dec(a), y = dec(b), big = fmax(fabs(x), fabs(y));
        chk(0, "add", a, b, dec(fz_add(a, b)), x + y, big);
        chk(1, "sub", a, b, dec(fz_sub(a, b)), x - y, big);
        chk(2, "mul", a, b, dec(fz_mul(a, b)), x * y, fabs(x * y));
        chk(3, "div", a, b, dec(fz_div(a, b)), x / y, fabs(x / y));
        int16_t k = (int16_t)rnd();
        chk(4, "i2f", a, b, dec(fz_from_i16(k)), (double)k, fabs((double)k));
        double v = ldexp((double)(rnd() & 0xFFFFFF) / 16777216.0 + 0.5, (int)(rnd() % 17)) * ((rnd() & 1) ? -1 : 1);
        fz w = 0;
        { int e; double mm = frexp(v, &e); /* v = mm * 2^e, mm in [.5,1) */ int32_t M = (int32_t)(mm * 4194304.0 * 2) ; w = ((uint32_t)M << 8) | (uint32_t)(e - 1 + 128); }
        int16_t t = fz_to_i16(w);
        double dv = dec(w);
        n[5]++;
        if (fabs(dv) < 32768 && t != (int16_t)trunc(dv) && fails++ < 20) printf("FAIL f2i w=%08x got=%d want=%d\n", w, t, (int)trunc(dv));
    }
    printf("n: add %ld sub %ld mul %ld div %ld i2f %ld f2i %ld  fails=%ld\n", n[0], n[1], n[2], n[3], n[4], n[5], fails);
    return fails != 0;
}
