/* 128-bit shifts vs C's operators, every count 0..127.
 * cc -O2 -fno-builtin -Iinclude '-DSFB_NAME(n)=sfb_##n' tests/host_int128.c lib/int/int128.c && ./a.out */
#include <stdio.h>
#include <stdint.h>
typedef unsigned __int128 u128; typedef __int128 i128;
u128 sfb_ashlti3(u128, int), sfb_lshrti3(u128, int);
i128 sfb_ashrti3(i128, int);
static uint64_t rng = 0x2545F4914F6CDD1Dull;
static uint64_t rnd(void) { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; return rng; }
int main(void)
{
    long fails = 0, n = 0;
    for (int i = 0; i < 20000; i++) {
        u128 x = ((u128)rnd() << 64) | rnd();
        if (i % 4 == 0) x >>= rnd() % 128;
        if (i % 7 == 0) x = ((u128)1 << (rnd() % 128)) - (i % 2);
        for (int b = 0; b < 128; b++) {
            n += 3;
            if (sfb_ashlti3(x, b) != x << b) fails++;
            if (sfb_lshrti3(x, b) != x >> b) fails++;
            if (sfb_ashrti3((i128)x, b) != (i128)x >> b) fails++;
        }
    }
    printf("n=%ld fails=%ld\n", n, fails);
    return fails != 0;
}
