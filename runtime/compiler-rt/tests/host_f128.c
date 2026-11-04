/* binary128 against the host's __float128 (libgcc's tf routines): the library contract of add, sub, mul, div, neg, unord and every compare entry, NaN included.
 * cc -O2 -fno-builtin -ffp-contract=off -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f128.c $(tests/fp_sources.sh f128) && ./a.out [n]
 * Operands: random bit patterns, a third of them drawn from the edges (zeros, infinities, NaNs, denormals, the largest and smallest normals, one apart). */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
typedef unsigned __int128 u128;
typedef __float128 q;
q sfb_addtf3(q, q), sfb_subtf3(q, q), sfb_multf3(q, q), sfb_divtf3(q, q), sfb_negtf2(q);
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
