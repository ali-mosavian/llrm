/* __clzhi2 __ctz{hi,si,di}2 __popcount{hi,si,di}2 against __builtin_*: every single-bit value, random values at every width, all ones, and each value's complement.
 * cc -O2 -Iinclude '-DSFB_NAME(n)=sfb_##n' tests/host_bitcount.c lib/int/clz{hi,si,di}2.c lib/int/ctz{hi,si,di}2.c lib/int/popcount{hi,si,di}2.c && ./a.out */
#include <stdint.h>
#include <stdio.h>

int sfb_clzhi2(uint16_t); int sfb_clzsi2(uint32_t); int sfb_clzdi2(uint64_t);
int sfb_ctzhi2(uint16_t); int sfb_ctzsi2(uint32_t); int sfb_ctzdi2(uint64_t);
int sfb_popcounthi2(uint16_t); int sfb_popcountsi2(uint32_t); int sfb_popcountdi2(uint64_t);

static long fails, n;
#define CHK(got, want) do { n++; if ((got) != (want)) { if (fails++ < 10) printf("FAIL line %d: %d != %d\n", __LINE__, (int)(got), (int)(want)); } } while (0)

static void one(uint64_t v)
{
    uint16_t h = (uint16_t)v; uint32_t w = (uint32_t)v;
    if (h) { CHK(sfb_clzhi2(h), __builtin_clz(h) - 16); CHK(sfb_ctzhi2(h), __builtin_ctz(h)); }
    if (w) { CHK(sfb_clzsi2(w), __builtin_clz(w)); CHK(sfb_ctzsi2(w), __builtin_ctz(w)); }
    if (v) { CHK(sfb_clzdi2(v), __builtin_clzll(v)); CHK(sfb_ctzdi2(v), __builtin_ctzll(v)); }
    CHK(sfb_popcounthi2(h), __builtin_popcount(h)); CHK(sfb_popcountsi2(w), __builtin_popcount(w)); CHK(sfb_popcountdi2(v), __builtin_popcountll(v));
}

int main(void)
{
    uint64_t r = 0x9E3779B97F4A7C15ull;
    for (int i = 0; i < 64; i++) { one(1ull << i); one(~(1ull << i)); one(~0ull << i); one(~0ull >> i); }
    one(0);
    for (long k = 0; k < 2000000; k++) {
        r ^= r << 13; r ^= r >> 7; r ^= r << 17;
        one(r >> (k % 64)); one(r << (k % 64));
    }
    printf("bitcount checks=%ld fails=%ld\n", n, fails);
    return fails != 0;
}
