/* Every compare entry point against C's operators, NaN included: the library contract (compiler-rt/libgcc) is
 * lt <0 iff a<b, le <=0 iff a<=b, ge >=0 iff a>=b, gt >0 iff a>b, eq ==0 iff a==b, ne !=0 iff a!=b, cmp as lt/le;
 * unordered: > 0 from lt/le/eq/ne/cmp, < 0 from ge/gt (ours: exactly +1 and -1), unord 1. host_f32.c only checked cmp and unord.
 * cc -O2 -fno-builtin -ffp-contract=off -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_cmp_entries.c lib/fp/f32/*.c lib/fp/f64/*.c && ./a.out [n] */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#define DECL(w, T) int sfb_cmp##w(T, T), sfb_eq##w(T, T), sfb_ne##w(T, T), sfb_lt##w(T, T), sfb_le##w(T, T), sfb_ge##w(T, T), sfb_gt##w(T, T), sfb_unord##w(T, T);
DECL(sf2, float) DECL(df2, double)
static uint64_t rng = 0x9E3779B97F4A7C15ull;
static uint64_t rnd(void) { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; return rng; }
static long fails, n;
#define CHECK(w, T, a, b) do { int un = (a) != (a) || (b) != (b); n++; \
    int c = sfb_cmp##w(a, b), e = sfb_eq##w(a, b), ne = sfb_ne##w(a, b), l = sfb_lt##w(a, b), le = sfb_le##w(a, b), ge = sfb_ge##w(a, b), g = sfb_gt##w(a, b); \
    int bad = sfb_unord##w(a, b) != un; \
    if (un) bad |= c != 1 || e != 1 || ne != 1 || l != 1 || le != 1 || ge != -1 || g != -1; \
    else bad |= (l < 0) != ((a) < (b)) || (le <= 0) != ((a) <= (b)) || (ge >= 0) != ((a) >= (b)) || (g > 0) != ((a) > (b)) || (e == 0) != ((a) == (b)) || (ne != 0) != ((a) != (b)) \
              || (c < 0) != ((a) < (b)) || (c <= 0) != ((a) <= (b)); \
    if (bad && fails++ < 20) printf("FAIL %s c=%d e=%d ne=%d l=%d le=%d ge=%d g=%d\n", #w, c, e, ne, l, le, ge, g); } while (0)
static const uint32_t e32[] = {0, 0x80000000u, 1, 0x80000001u, 0x3F800000u, 0xBF800000u, 0x7F7FFFFFu, 0xFF7FFFFFu, 0x7F800000u, 0xFF800000u, 0x7FC00000u, 0xFFC00000u, 0x7F800001u, 0xFF800001u, 0x00800000u, 0x007FFFFFu};
int main(int argc, char **argv)
{
    long cnt = argc > 1 ? atol(argv[1]) : 2000000;
    for (long i = 0; i < cnt; i++) {
        uint32_t x = i % 4 == 0 ? e32[rnd() % 16] : (uint32_t)rnd(), y = i % 3 == 0 ? e32[rnd() % 16] : (uint32_t)rnd();
        float a, b; memcpy(&a, &x, 4); memcpy(&b, &y, 4); CHECK(sf2, float, a, b);
        uint64_t p = i % 4 == 0 ? ((uint64_t)e32[rnd() % 16] << 32) | (rnd() & 1 ? 0 : (uint32_t)rnd()) : rnd(), q = i % 3 == 0 ? ((uint64_t)e32[rnd() % 16] << 32) | (rnd() & 1 ? 0 : (uint32_t)rnd()) : rnd();
        double d, f; memcpy(&d, &p, 8); memcpy(&f, &q, 8); CHECK(df2, double, d, f);
    }
    printf("n=%ld fails=%ld\n", n, fails); return fails != 0;
}
