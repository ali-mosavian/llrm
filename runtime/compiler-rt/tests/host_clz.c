/* __clzsi2 and __clzdi2 against __builtin_clz{,ll}: every single-bit value, random values at every width, all ones.
 * cc -O2 -Iinclude '-DSFB_NAME(n)=sfb_##n' tests/host_clz.c lib/int/clzsi2.c lib/int/clzdi2.c && ./a.out */
#include <stdint.h>
#include <stdio.h>

int sfb_clzsi2(uint32_t);
int sfb_clzdi2(uint64_t);

int main(void)
{
    uint64_t r = 0x9E3779B97F4A7C15ull;
    long fails = 0, n = 0;
    for (int i = 0; i < 64; i++) {
        uint64_t v = 1ull << i;
        if (i < 32) { fails += sfb_clzsi2((uint32_t)v) != __builtin_clz((uint32_t)v); n++; }
        fails += sfb_clzdi2(v) != __builtin_clzll(v); n++;
    }
    for (long k = 0; k < 2000000; k++) {
        r ^= r << 13; r ^= r >> 7; r ^= r << 17;
        uint64_t v = r >> (k % 64);
        if (!v) continue;
        if ((uint32_t)v) { fails += sfb_clzsi2((uint32_t)v) != __builtin_clz((uint32_t)v); n++; }
        fails += sfb_clzdi2(v) != __builtin_clzll(v); n++;
    }
    fails += sfb_clzsi2(0xFFFFFFFFu) != 0; fails += sfb_clzdi2(~0ull) != 0;
    printf("clz checks=%ld fails=%ld\n", n, fails);
    return fails != 0;
}
