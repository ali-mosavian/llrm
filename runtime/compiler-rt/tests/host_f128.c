/* binary128 against the host's __float128 (libgcc's tf routines): the library contract of add, sub, mul, div, neg, unord and every compare entry, NaN included.
 * cc -O2 -fno-builtin -ffp-contract=off -DSF_QUAD_FLOAT128 -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f128.c $(tests/fp_sources.sh f128) lib/fp/f32/pack.c lib/fp/f64/pack.c && ./a.out [n]
 * Operands: random bit patterns, a third of them drawn from the edges (zeros, infinities, NaNs, denormals, the largest and smallest normals, one apart). */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
typedef unsigned __int128 u128;
typedef __float128 q;
q sfb_addtf3(q, q), sfb_subtf3(q, q), sfb_multf3(q, q), sfb_divtf3(q, q), sfb_negtf2(q);
q sfb_extendsftf2(float), sfb_extenddftf2(double), sfb_floatsitf(int32_t), sfb_floatunsitf(uint32_t), sfb_floatditf(int64_t), sfb_floatunditf(uint64_t), sfb_floattitf(__int128), sfb_floatuntitf(u128);
float sfb_trunctfsf2(q);
double sfb_trunctfdf2(q);
int32_t sfb_fixtfsi(q);
uint32_t sfb_fixunstfsi(q);
int64_t sfb_fixtfdi(q);
uint64_t sfb_fixunstfdi(q);
__int128 sfb_fixtfti(q);
u128 sfb_fixunstfti(q);
int sfb_cmptf2(q, q), sfb_eqtf2(q, q), sfb_netf2(q, q), sfb_lttf2(q, q), sfb_letf2(q, q), sfb_getf2(q, q), sfb_gttf2(q, q), sfb_unordtf2(q, q);
static uint64_t rng = 0x9E3779B97F4A7C15ull;
static uint64_t rnd(void) { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; return rng; }
static u128 wide(void) { return ((u128)rnd() << 64) | rnd(); }
static u128 edge(void)
{
    static const u128 top[] = {0, (u128)1 << 127, (u128)0x7FFF << 112, ((u128)0xFFFF << 112), ((u128)0x7FFF8 << 108), ((u128)0xFFFF8 << 108), ((u128)0x7FFF << 112) | 1,
        ((u128)1 << 112), ((u128)1 << 112) - 1, 1, ((u128)0x7FFE << 112) | (((u128)1 << 112) - 1), (u128)0x3FFF << 112};
    u128 x = top[rnd() % (sizeof top / sizeof *top)];
    switch (rnd() % 4) { case 0: x += rnd() % 3; break; case 1: x ^= (u128)1 << 127; break; case 2: x ^= (u128)1 << (rnd() % 112); break; }
    return x;
}
static q f(u128 b) { q x; memcpy(&x, &b, 16); return x; }
static u128 bits(q x) { u128 b; memcpy(&b, &x, 16); return b; }
static int isnan_(u128 b) { return ((b << 1) >> 1 >> 112) == 0x7FFF && (b & (((u128)1 << 112) - 1)); }
static long fails, n;
static void bad(const char *what, u128 a, u128 b, u128 got, u128 want)
{
    if (fails++ < 20) printf("FAIL %s a=%016llx%016llx b=%016llx%016llx got=%016llx%016llx want=%016llx%016llx\n", what, (unsigned long long)(a >> 64), (unsigned long long)a, (unsigned long long)(b >> 64), (unsigned long long)b,
        (unsigned long long)(got >> 64), (unsigned long long)got, (unsigned long long)(want >> 64), (unsigned long long)want);
}
/* A NaN result may carry any payload as long as it is quiet and a NaN; everything else is bit-exact. */
static void arith(const char *what, u128 a, u128 b, u128 got, u128 want)
{
    n++;
    if (isnan_(want)) { if (!isnan_(got) || !((got >> 111) & 1)) bad(what, a, b, got, want); }
    else if (got != want) bad(what, a, b, got, want);
}
int main(int argc, char **argv)
{
    long cnt = argc > 1 ? atol(argv[1]) : 2000000;
    for (long i = 0; i < cnt; i++) {
        u128 x = i % 3 == 0 ? edge() : wide(), y = i % 3 == 1 ? edge() : wide();
        if (i % 5 == 0) y = x ^ ((u128)(rnd() % 4) << (rnd() % 112)); /* nearby operands: cancellation, equal exponents */
        q a = f(x), b = f(y);
        arith("add", x, y, bits(sfb_addtf3(a, b)), bits(a + b));
        arith("sub", x, y, bits(sfb_subtf3(a, b)), bits(a - b));
        arith("mul", x, y, bits(sfb_multf3(a, b)), bits(a * b));
        arith("div", x, y, bits(sfb_divtf3(a, b)), bits(a / b));
        n++; if (bits(sfb_negtf2(a)) != (x ^ ((u128)1 << 127))) bad("neg", x, y, bits(sfb_negtf2(a)), x ^ ((u128)1 << 127)); /* bit-exact, a signalling NaN stays one */
        /* conversions out of binary128: in range and not NaN only (C leaves the rest undefined) */
        /* in-range operands for the float to integer entries: a random integer over a small divisor, so fractions and every width occur */
        q aq = (q)(__int128)(wide() >> (rnd() % 128)) / (q)(1 + rnd() % 1000);
        if (rnd() & 1) aq = -aq;
        for (int pass = 0; pass < 2; pass++) {
        q a = pass ? aq : f(x);
        if (a == a) {
            float tf = (float)a; double td = (double)a;
            n += 2;
            if (a != a || sfb_trunctfsf2(a) != tf) bad("trunc32", x, y, 0, 0);
            if (sfb_trunctfdf2(a) != td) bad("trunc64", x, y, 0, 0);
            q lo = a < 0 ? -a : a;
            if (lo < 2147483648.0Q) { n++; if (sfb_fixtfsi(a) != (int32_t)a) bad("fixsi", x, y, 0, 0); }
            if (lo < 4294967296.0Q) { n++; if (a > -1 && sfb_fixunstfsi(a) != (uint32_t)a) bad("fixusi", x, y, 0, 0); }
            if (lo < 9223372036854775808.0Q) { n++; if (sfb_fixtfdi(a) != (int64_t)a) bad("fixdi", x, y, 0, 0); }
            if (lo < 18446744073709551616.0Q) { n++; if (a > -1 && sfb_fixunstfdi(a) != (uint64_t)a) bad("fixudi", x, y, 0, 0); }
            if (lo < 1.7014118346046923173168730371588410572e38Q) { n++; if (sfb_fixtfti(a) != (__int128)a) bad("fixti", x, y, 0, 0); }
            if (lo < 3.4028236692093846346337460743176821145e38Q) { n++; if (a > -1 && sfb_fixunstfti(a) != (u128)a) bad("fixuti", x, y, 0, 0); }
        }
        }
        {
            uint32_t w = (uint32_t)rnd(); uint64_t v = rnd(); u128 t = wide() >> (rnd() % 128);
            float fw; memcpy(&fw, &w, 4); double dv; memcpy(&dv, &v, 8);
            n += 8;
            arith("extsf", w, 0, bits(sfb_extendsftf2(fw)), bits((q)fw));
            arith("extdf", v, 0, bits(sfb_extenddftf2(dv)), bits((q)dv));
            arith("floatsi", (int32_t)w, 0, bits(sfb_floatsitf((int32_t)w)), bits((q)(int32_t)w));
            arith("floatusi", w, 0, bits(sfb_floatunsitf(w)), bits((q)w));
            arith("floatdi", v, 0, bits(sfb_floatditf((int64_t)v)), bits((q)(int64_t)v));
            arith("floatudi", v, 0, bits(sfb_floatunditf(v)), bits((q)v));
            arith("floatti", t, 0, bits(sfb_floattitf((__int128)t)), bits((q)(__int128)t));
            arith("floatuti", t, 0, bits(sfb_floatuntitf(t)), bits((q)t));
        }
        int un = a != a || b != b;
        n++;
        int c = sfb_cmptf2(a, b), e = sfb_eqtf2(a, b), ne = sfb_netf2(a, b), l = sfb_lttf2(a, b), le = sfb_letf2(a, b), ge = sfb_getf2(a, b), g = sfb_gttf2(a, b);
        int badc = sfb_unordtf2(a, b) != un;
        if (un) badc |= c != 1 || e != 1 || ne != 1 || l != 1 || le != 1 || ge != -1 || g != -1;
        else badc |= (l < 0) != (a < b) || (le <= 0) != (a <= b) || (ge >= 0) != (a >= b) || (g > 0) != (a > b) || (e == 0) != (a == b) || (ne != 0) != (a != b) || (c < 0) != (a < b) || (c <= 0) != (a <= b);
        if (badc) bad("cmp", x, y, (u128)(unsigned)c, (u128)un);
    }
    printf("n=%ld fails=%ld\n", n, fails);
    return fails != 0;
}
