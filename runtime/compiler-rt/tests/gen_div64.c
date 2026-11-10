/* Vectors for m68k_div64.sh: dividend, divisor, then the four expected results. d == 0 as lib/int: q = ~0, r = n (signed: ~0 and n). */
#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>
static uint64_t s = 88172645463325252ull;
static uint64_t rnd(void) { s ^= s << 13; s ^= s >> 7; s ^= s << 17; return s; }
static uint64_t mag(void) { int b = rnd() % 65; uint64_t m = b == 64 ? ~0ull : (1ull << b) - 1; return rnd() & m; }
static void row(uint64_t n, uint64_t d)
{
    uint64_t uq = d ? n / d : ~0ull, ur = d ? n % d : n, sq, sr;
    int na = (int64_t)n < 0, nb = (int64_t)d < 0;
    uint64_t an = na ? -n : n, ad = nb ? -d : d, q = ad ? an / ad : ~0ull, r = ad ? an % ad : an;
    sq = (na != nb) && d ? -q : q; sr = na ? -r : r;
    printf("{0x%08xu,0x%08xu,0x%08xu,0x%08xu,0x%08xu,0x%08xu,0x%08xu,0x%08xu,0x%08xu,0x%08xu,0x%08xu,0x%08xu},\n", (unsigned)(n >> 32), (unsigned)n, (unsigned)(d >> 32), (unsigned)d,
           (unsigned)(uq >> 32), (unsigned)uq, (unsigned)(ur >> 32), (unsigned)ur, (unsigned)(sq >> 32), (unsigned)sq, (unsigned)(sr >> 32), (unsigned)sr);
}
int main(int argc, char **argv)
{
    static const uint64_t e[] = {0, 1, 2, 3, 0xFFFF, 0x10000, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF, 0x100000000ull, 0x100000001ull, 0x1FFFFFFFFull, 0x7FFFFFFF00000000ull,
                                 0x7FFFFFFFFFFFFFFFull, 0x8000000000000000ull, 0x8000000000000001ull, 0xFFFFFFFE00000001ull, 0xFFFFFFFF00000000ull, 0xFFFFFFFFFFFFFFFEull, ~0ull};
    int n = argc > 1 ? atoi(argv[1]) : 3000;
    printf("#define NEDGE 400u\n#define NRND %du\nstatic const unsigned V[][12] = {\n", n);
    for (unsigned i = 0; i < sizeof e / sizeof *e; i++) for (unsigned j = 0; j < sizeof e / sizeof *e; j++) row(e[i], e[j]);
    for (int i = 0; i < n; i++) row(mag(), mag());
    /* d = 2^k:(2^m - 1) with n = d*q + r near the quotient and remainder limits: the shapes where a divide estimate is furthest off */
    static const uint64_t qs[] = {0xFFFFFFFF, 0xFFFFFFFE, 0x80000000, 0x7FFFFFFF, 0x10000, 2, 1};
    for (int k = 0; k < 32; k++)
        for (int m = 0; m < 4; m++) {
            uint64_t d = (1ull << (32 + k)) | (m == 0 ? 0 : m == 1 ? 1 : m == 2 ? 0x7FFFFFFF : 0xFFFFFFFF);
            if (k == 31) d = (d & 0xFFFFFFFF) | (1ull << 63);
            for (unsigned i = 0; i < sizeof qs / sizeof *qs; i++) {
                unsigned __int128 b = (unsigned __int128)d * qs[i];
                uint64_t rs[] = {0, 1, d / 2, d - 1};
                for (int j = 0; j < 4; j++) { unsigned __int128 v = b + rs[j]; if (!(v >> 64)) row((uint64_t)v, d); }
            }
        }
    printf("};\n");
    return 0;
}
