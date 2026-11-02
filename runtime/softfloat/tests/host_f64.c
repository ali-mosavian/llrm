/* Bit-exact oracle: soft f64 vs the host FPU on edge classes and random sweeps (NaN compared as a class).
 * cc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f64.c src/sf64.c [n] */
#define SF_NAME(n) sfb_##n
#include "softfloat.h"
#include <stdio.h>
#include <stdlib.h>
double sfb_adddf3(double, double), sfb_subdf3(double, double), sfb_muldf3(double, double), sfb_divdf3(double, double);
int sfb_cmpdf2(double, double), sfb_gedf2(double, double), sfb_unorddf2(double, double);
double sfb_floatsidf(int32_t), sfb_floatunsidf(uint32_t), sfb_floatdidf(int64_t), sfb_floatundidf(uint64_t);
int32_t sfb_fixdfsi(double); uint32_t sfb_fixunsdfsi(double); int64_t sfb_fixdfdi(double); uint64_t sfb_fixunsdfdi(double);

static uint64_t rng = 0x9E3779B97F4A7C15ull;
static uint64_t rnd(void)
{
    rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
    return rng * 0x2545F4914F6CDD1Dull;
}
static int nan64(uint64_t b) { return (b << 1) > 0xFFE0000000000000ull; }

static uint64_t edge[] = {
    0x0000000000000000ull, 0x8000000000000000ull, 1, 0x8000000000000001ull, 0x000FFFFFFFFFFFFFull, 0x0010000000000000ull,
    0x0010000000000001ull, 0x3FF0000000000000ull, 0xBFF0000000000000ull, 0x3FF0000000000001ull, 0x3FEFFFFFFFFFFFFFull,
    0x4000000000000000ull, 0x7FEFFFFFFFFFFFFFull, 0xFFEFFFFFFFFFFFFFull, 0x7FF0000000000000ull, 0xFFF0000000000000ull,
    0x7FF8000000000000ull, 0x7FF0000000000001ull, 0xFFF8000000000001ull, 0x4330000000000000ull, 0x433FFFFFFFFFFFFFull,
    0x43E0000000000000ull, 0xC3E0000000000000ull, 0x41E0000000000000ull, 0xC1E0000000000000ull, 0x3FE0000000000000ull,
    0x3CA0000000000000ull, 0x7FD0000000000000ull, 0x0020000000000000ull, 0x400921FB54442D18ull,
};
#define NEDGE (sizeof edge / sizeof edge[0])

static uint64_t gen(void)
{
    uint64_t r = rnd();
    switch (r & 7) {
    case 0: return edge[(r >> 8) % NEDGE];
    case 1: return rnd() & 0x800FFFFFFFFFFFFFull;
    case 2: return (rnd() & 0x800FFFFFFFFFFFFFull) | ((uint64_t)(1000 + (rnd() & 0x3F)) << 52);
    case 3: return edge[(r >> 8) % NEDGE] + (rnd() & 7) - 3;
    default: return rnd();
    }
}

static long fails, count[16];
static const char *opname[] = {"add", "sub", "mul", "div", "cmp", "i32->d", "u32->d", "i64->d", "u64->d",
                               "d->i32", "d->u32", "d->i64", "d->u64"};

static void check(int op, uint64_t a, uint64_t b, uint64_t got, uint64_t want, int isd)
{
    count[op]++;
    int ok = got == want || (isd && nan64(got) && nan64(want));
    if (!ok && fails++ < 40)
        printf("FAIL %s a=%llx b=%llx got=%llx want=%llx\n", opname[op], (unsigned long long)a,
               (unsigned long long)b, (unsigned long long)got, (unsigned long long)want);
}

static void binops(uint64_t a, uint64_t b)
{
    volatile double x = b2d(a), y = b2d(b);
    check(0, a, b, d2b(sfb_adddf3(x, y)), d2b(x + y), 1);
    check(1, a, b, d2b(sfb_subdf3(x, y)), d2b(x - y), 1);
    check(2, a, b, d2b(sfb_muldf3(x, y)), d2b(x * y), 1);
    check(3, a, b, d2b(sfb_divdf3(x, y)), d2b(x / y), 1);
    int want = x < y ? -1 : x > y ? 1 : x == y ? 0 : 2;
    int got = sfb_unorddf2(x, y) ? 2 : sfb_cmpdf2(x, y);
    check(4, a, b, got, want, 0);
    if (want == 2) check(4, a, b, sfb_gedf2(x, y), (uint64_t)(int64_t)-1, 0);
}

static void toint(uint64_t a)
{
    volatile double x = b2d(a);
    if (x > -2147483649.0 && x < 2147483648.0) check(9, a, 0, (uint32_t)sfb_fixdfsi(x), (uint32_t)(int32_t)x, 0);
    if (x > -1.0 && x < 4294967296.0) check(10, a, 0, sfb_fixunsdfsi(x), (uint32_t)x, 0);
    if (x > -9223372036854775808.0 && x < 9223372036854775808.0) check(11, a, 0, sfb_fixdfdi(x), (uint64_t)(int64_t)x, 0);
    if (x == -9223372036854775808.0) check(11, a, 0, sfb_fixdfdi(x), 0x8000000000000000ull, 0);
    if (x > -1.0 && x < 18446744073709551616.0) check(12, a, 0, sfb_fixunsdfdi(x), (uint64_t)x, 0);
}

static void fromint(uint64_t v)
{
    volatile int32_t i = (int32_t)v; volatile uint32_t u = (uint32_t)v;
    volatile int64_t l = (int64_t)v; volatile uint64_t ul = v;
    check(5, v, 0, d2b(sfb_floatsidf(i)), d2b((double)i), 1);
    check(6, v, 0, d2b(sfb_floatunsidf(u)), d2b((double)u), 1);
    check(7, v, 0, d2b(sfb_floatdidf(l)), d2b((double)l), 1);
    check(8, v, 0, d2b(sfb_floatundidf(ul)), d2b((double)ul), 1);
}

int main(int argc, char **argv)
{
    long n = argc > 1 ? atol(argv[1]) : 5000000;
    for (unsigned i = 0; i < NEDGE; i++) {
        for (unsigned j = 0; j < NEDGE; j++) binops(edge[i], edge[j]);
        toint(edge[i]);
    }
    for (long i = 0; i < n; i++) { binops(gen(), gen()); toint(gen()); }
    /* products and quotients that land on ties: equal exponents with few set bits */
    for (long i = 0; i < n / 4; i++) {
        uint64_t e = (uint64_t)(1023 - 30 + (rnd() % 60)) << 52;
        binops(e | (rnd() & 0xFFFFFFFFull) << 20, (uint64_t)(1023 - 30 + (rnd() % 60)) << 52 | (rnd() & 0xFFFFFFFFull) << 20);
    }
    for (int s = 0; s < 64; s++)
        for (int d = -4; d <= 4; d++) { fromint((1ull << s) + d); fromint(~((1ull << s) + d) + 1); }
    for (uint64_t v = 0; v < 70000; v++) fromint(v), fromint(0ull - v);
    for (long i = 0; i < n; i++) fromint(rnd() >> (rnd() & 63));
    /* 64-bit ties: the 11 bits below the 53-bit mantissa at exactly half, half+-1 */
    for (int s = 53; s < 64; s++)
        for (long i = 0; i < 2000; i++) {
            uint64_t v = (1ull << s) | (rnd() & ((1ull << s) - 1)), half = 1ull << (s - 53), mask = (half << 1) - 1;
            for (int d = -1; d <= 1; d++) { uint64_t t = (v & ~mask) | (half + d); fromint(t); fromint(0ull - t); }
        }
    for (int i = 0; i < 13; i++) printf("%-7s %ld\n", opname[i], count[i]);
    printf("fails=%ld\n", fails);
    return fails != 0;
}
