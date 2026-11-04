/* floatsidf/floatunsidf take the exact 32-bit path (conv.h from_mag32): every exponent position, +-1 and the all-ones values, against the FPU.
 * cc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f64_float32.c lib/fp/f64/float.c lib/fp/f64/pack.c && ./a.out */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
double sfb_floatsidf(int32_t), sfb_floatunsidf(uint32_t);
static uint64_t bits(double d) { uint64_t u; memcpy(&u, &d, 8); return u; }
int main(void)
{
    int fails = 0;
    for (int k = 0; k < 32; ++k)
        for (int d = -2; d <= 2; ++d) {
            uint32_t u = (uint32_t)((1ull << k) + d), s = (uint32_t)(0u - u);
            if (bits(sfb_floatunsidf(u)) != bits((double)u)) { printf("u %u\n", u); ++fails; }
            if (bits(sfb_floatsidf((int32_t)u)) != bits((double)(int32_t)u)) { printf("s %d\n", (int32_t)u); ++fails; }
            if (bits(sfb_floatsidf((int32_t)s)) != bits((double)(int32_t)s)) { printf("s %d\n", (int32_t)s); ++fails; }
        }
    printf("fails=%d\n", fails);
    return fails != 0;
}
