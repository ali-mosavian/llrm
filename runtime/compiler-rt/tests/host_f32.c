/* Bit-exact oracle: soft f32 vs the host FPU on edge classes and random sweeps.
 * cc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f32.c lib/fp/sf32.c lib/fp/sf32_conv.c lib/fp/sf32_conv64.c */
#define SF_NAME(n) sfb_##n
#include "softfloat.h"
float sfb_addsf3(float, float), sfb_subsf3(float, float), sfb_mulsf3(float, float), sfb_divsf3(float, float);
int sfb_cmpsf2(float, float), sfb_unordsf2(float, float);
float sfb_floatsisf(int32_t), sfb_floatunsisf(uint32_t), sfb_floatdisf(int64_t), sfb_floatundisf(uint64_t);
int32_t sfb_fixsfsi(float); uint32_t sfb_fixunssfsi(float); int64_t sfb_fixsfdi(float); uint64_t sfb_fixunssfdi(float);
double sfb_extendsfdf2(float); float sfb_truncdfsf2(double);
#define sf32_add(a, b) f2b(sfb_addsf3(b2f(a), b2f(b)))
#define sf32_sub(a, b) f2b(sfb_subsf3(b2f(a), b2f(b)))
#define sf32_mul(a, b) f2b(sfb_mulsf3(b2f(a), b2f(b)))
#define sf32_div(a, b) f2b(sfb_divsf3(b2f(a), b2f(b)))
#define sf32_from_i32(x) f2b(sfb_floatsisf(x))
#define sf32_from_u32(x) f2b(sfb_floatunsisf(x))
#define sf32_from_i64(x) f2b(sfb_floatdisf(x))
#define sf32_from_u64(x) f2b(sfb_floatundisf(x))
#define sf32_to_i32(a) sfb_fixsfsi(b2f(a))
#define sf32_to_u32(a) sfb_fixunssfsi(b2f(a))
#define sf32_to_i64(a) sfb_fixsfdi(b2f(a))
#define sf32_to_u64(a) sfb_fixunssfdi(b2f(a))
#define sf32_to_sf64(a) d2b(sfb_extendsfdf2(b2f(a)))
#define sf64_to_sf32(a) f2b(sfb_truncdfsf2(b2d(a)))
/* compare: ordered result from __cmpsf2, unordered from __unordsf2 */
static int sf32_cmp(uint32_t a, uint32_t b) { return sfb_unordsf2(b2f(a), b2f(b)) ? 2 : sfb_cmpsf2(b2f(a), b2f(b)); }
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t rng = 0x9E3779B97F4A7C15ull;
static uint32_t rnd(void)
{
    rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
    return (uint32_t)(rng >> 16);
}

static int nan32(uint32_t b) { return (b << 1) > 0xFF000000u; }
static int nan64(uint64_t b) { return (b << 1) > 0xFFE0000000000000ull; }

static uint32_t edge[] = {
    0x00000000, 0x80000000, 0x00000001, 0x80000001, 0x007FFFFF, 0x00800000, 0x00800001, 0x3F800000,
    0xBF800000, 0x3F800001, 0x3F7FFFFF, 0x40000000, 0x7F7FFFFF, 0xFF7FFFFF, 0x7F800000, 0xFF800000,
    0x7FC00000, 0x7F800001, 0xFFC00001, 0x4B000000, 0x4B7FFFFF, 0x4B800000, 0x4F000000, 0xCF000000,
    0x3F000000, 0x34000000, 0x33800000, 0x7E800000, 0x01000000, 0x40490FDB,
};
#define NEDGE (sizeof edge / sizeof edge[0])

/* Random pattern from a mix of classes: uniform bits, near-equal exponents, denormals, extremes. */
static uint32_t gen(void)
{
    uint32_t r = rnd();
    switch (r & 7) {
    case 0: return edge[(r >> 8) % NEDGE];
    case 1: return rnd() & 0x807FFFFFu;
    case 2: return (rnd() & 0x80000000u) | (rnd() & 0x7FFFFFu) | ((60 + (rnd() & 0x3F)) << 23);
    case 3: return edge[(r >> 8) % NEDGE] + (rnd() & 7) - 3;
    default: return rnd();
    }
}

static long fails, count[16];
static const char *opname[] = {"add", "sub", "mul", "div", "cmp", "i32->f", "u32->f", "i64->f", "u64->f",
                               "f->i32", "f->u32", "f->i64", "f->u64", "ext", "trunc"};

static void check(int op, uint64_t a, uint64_t b, uint64_t got, uint64_t want, int wide)
{
    count[op]++;
    int ok = wide ? (got == want || (nan64(got) && nan64(want))) : (got == want || (nan32(got) && nan32(want)));
    if (!ok && fails++ < 40)
        printf("FAIL %s a=%llx b=%llx got=%llx want=%llx\n", opname[op], (unsigned long long)a,
               (unsigned long long)b, (unsigned long long)got, (unsigned long long)want);
}

static void binops(uint32_t a, uint32_t b)
{
    volatile float x = b2f(a), y = b2f(b);
    check(0, a, b, sf32_add(a, b), f2b(x + y), 0);
    check(1, a, b, sf32_sub(a, b), f2b(x - y), 0);
    check(2, a, b, sf32_mul(a, b), f2b(x * y), 0);
    check(3, a, b, sf32_div(a, b), f2b(x / y), 0);
    int want = x < y ? -1 : x > y ? 1 : x == y ? 0 : 2;
    check(4, a, b, sf32_cmp(a, b), want, 0);
}

static void toint(uint32_t a)
{
    volatile float x = b2f(a);
    if (x > -2147483648.0f && x < 2147483648.0f) check(9, a, 0, (uint32_t)sf32_to_i32(a), (uint32_t)(int32_t)x, 0);
    if (x > -1.0f && x < 4294967296.0f) check(10, a, 0, sf32_to_u32(a), (uint32_t)x, 0);
    if (x > -9223372036854775808.0f && x < 9223372036854775808.0f) check(11, a, 0, sf32_to_i64(a), (uint64_t)(int64_t)x, 1);
    if (x > -1.0f && x < 18446744073709551616.0f) check(12, a, 0, sf32_to_u64(a), (uint64_t)x, 1);
    check(13, a, 0, sf32_to_sf64(a), d2b((double)x), 1);
}

static void fromint(uint64_t v)
{
    volatile int32_t i = (int32_t)v; volatile uint32_t u = (uint32_t)v;
    volatile int64_t l = (int64_t)v; volatile uint64_t ul = v;
    check(5, v, 0, sf32_from_i32(i), f2b((float)i), 0);
    check(6, v, 0, sf32_from_u32(u), f2b((float)u), 0);
    check(7, v, 0, sf32_from_i64(l), f2b((float)l), 0);
    check(8, v, 0, sf32_from_u64(ul), f2b((float)ul), 0);
}

static void trunc64(uint64_t d)
{
    volatile double x = b2d(d);
    check(14, d, 0, sf64_to_sf32(d), f2b((float)x), 0);
}

int main(int argc, char **argv)
{
    long n = argc > 1 ? atol(argv[1]) : 20000000;
    for (unsigned i = 0; i < NEDGE; i++) {
        for (unsigned j = 0; j < NEDGE; j++) binops(edge[i], edge[j]);
        toint(edge[i]);
    }
    for (long i = 0; i < n; i++) { binops(gen(), gen()); toint(gen()); }
    /* Exhaustive f32 -> f64 / ints over every pattern would be 2^32: stride sweep keeps it to 2^24. */
    for (uint64_t a = 0; a < 0x100000000ull; a += 251) toint((uint32_t)a);
    /* int -> f32: every power of two +- 1, tie neighbours, and random. */
    for (int s = 0; s < 64; s++)
        for (int d = -4; d <= 4; d++) { fromint((1ull << s) + d); fromint(~((1ull << s) + d) + 1); }
    for (uint64_t v = 0; v < 70000; v++) fromint(v), fromint(0ull - v);
    for (long i = 0; i < n; i++) { uint64_t v = ((uint64_t)rnd() << 32 | rnd()) >> (rnd() & 63); fromint(v); }
    /* f64 -> f32: random doubles, including denormal results and ties. */
    for (long i = 0; i < n; i++) {
        uint64_t d = (uint64_t)rnd() << 32 | rnd();
        if (i & 1) d = (d & 0x800FFFFFFFFFFFFFull) | ((uint64_t)(896 - 30 + (rnd() % 200)) << 52);
        trunc64(d);
        trunc64((d & 0xFFFFFFFFE0000000ull) | 0x10000000ull);
    }
    /* f32 -> f64 -> f32 must return the input (NaN as a class). */
    for (long i = 0; i < n; i++) {
        uint32_t a = gen();
        check(14, a, 0, sf64_to_sf32(sf32_to_sf64(a)), a, 0);
    }
    /* 64-bit ties: values with the 40 bits below the f32 mantissa at exactly half, half+-1. */
    for (int s = 24; s < 64; s++)
        for (long i = 0; i < 2000; i++) {
            uint64_t v = (1ull << s) | (((uint64_t)rnd() << 32 | rnd()) & ((1ull << s) - 1));
            uint64_t half = 1ull << (s - 24), mask = (half << 1) - 1;
            for (int d = -1; d <= 1; d++) {
                uint64_t t = (v & ~mask) | (half + d);
                fromint(t);
                fromint(0ull - t);
            }
        }
    for (int i = 0; i < 15; i++) printf("%-7s %ld\n", opname[i], count[i]);
    printf("fails=%ld\n", fails);
    return fails != 0;
}
