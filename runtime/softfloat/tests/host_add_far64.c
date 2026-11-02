/* f64 twin of host_add_far.c: exponents differ by 48..70, a = 2^k against 2^-d, 1.5 * 2^-d, all-ones mantissa, random; both signs, add and sub.
 * cc -O2 -ffp-contract=off -fno-builtin -DSF_INTABI -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_add_far64.c src/sf64.c src/sf32_conv.c */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
uint64_t sfb_adddf3(uint64_t, uint64_t);
uint64_t sfb_subdf3(uint64_t, uint64_t);
static double f(uint64_t u) { double x; memcpy(&x, &u, 8); return x; }
static uint64_t u(double x) { uint64_t v; memcpy(&v, &x, 8); return v; }
int main(void)
{
    long fails = 0, n = 0;
    srand(11);
    uint64_t mant[] = {0, 0xFFFFFFFFFFFFFull, 0x8000000000000ull, 1};
    for (int ea = 200; ea < 210; ea++) for (int d = 48; d <= 70; d++) for (int k = 0; k < 40; k++) for (int sg = 0; sg < 4; sg++) {
        uint64_t r1 = ((uint64_t)rand() << 31 ^ (uint64_t)rand() << 12 ^ rand()) & 0xFFFFFFFFFFFFFull, r2 = ((uint64_t)rand() << 31 ^ (uint64_t)rand() << 12 ^ rand()) & 0xFFFFFFFFFFFFFull;
        uint64_t ma = k < 16 ? mant[k >> 2] : r1, mb = k < 16 ? mant[k & 3] : r2;
        uint64_t a = ((uint64_t)(sg & 1) << 63) | ((uint64_t)ea << 52) | ma, b = ((uint64_t)(sg >> 1) << 63) | ((uint64_t)(ea - d) << 52) | mb;
        for (int sw = 0; sw < 2; sw++) {
            uint64_t x = sw ? b : a, y = sw ? a : b;
            n += 2;
            if (sfb_adddf3(x, y) != u(f(x) + f(y))) fails++;
            if (sfb_subdf3(x, y) != u(f(x) - f(y))) fails++;
        }
    }
    printf("far add/sub f64: %ld cases, fails=%ld\n", n, fails);
    return fails != 0;
}
