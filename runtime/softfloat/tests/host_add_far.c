/* Operands whose exponents differ by 20..40 (powers of two, 0x7FFFFF mantissas, random; both signs, add and sub): the far-exponent early
 * return of add_bits must give the host FPU's result. cc -O2 -ffp-contract=off -fno-builtin -DSF_INTABI -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_add_far.c src/sf32.c src/sf32_conv.c */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
uint32_t sfb_addsf3(uint32_t, uint32_t);
uint32_t sfb_subsf3(uint32_t, uint32_t);
static float f(uint32_t u) { float x; memcpy(&x, &u, 4); return x; }
static uint32_t u(float x) { uint32_t v; memcpy(&v, &x, 4); return v; }
int main(void)
{
    long fails = 0, n = 0;
    srand(11);
    uint32_t mant[] = {0, 0x7FFFFF, 0x400000, 1};
    for (int ea = 60; ea < 70; ea++) for (int d = 20; d <= 40; d++) for (int k = 0; k < 40; k++) for (int sg = 0; sg < 4; sg++) {
        uint32_t ma = k < 16 ? mant[k >> 2] : (rand() & 0x7FFFFF), mb = k < 16 ? mant[k & 3] : (rand() & 0x7FFFFF);
        uint32_t a = ((sg & 1) << 31) | ((uint32_t)ea << 23) | ma, b = ((sg >> 1) << 31) | ((uint32_t)(ea - d) << 23) | mb;
        for (int sw = 0; sw < 2; sw++) {
            uint32_t x = sw ? b : a, y = sw ? a : b;
            n += 2;
            if (sfb_addsf3(x, y) != u(f(x) + f(y))) fails++;
            if (sfb_subsf3(x, y) != u(f(x) - f(y))) fails++;
        }
    }
    printf("far add/sub: %ld cases, fails=%ld\n", n, fails);
    return fails != 0;
}
