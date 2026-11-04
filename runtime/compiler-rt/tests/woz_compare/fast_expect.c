/* Host: prints W[] (the results the target must reproduce bit for bit), -DOP as in fast_main.c. */
#include <stdint.h>
#include <stdio.h>
#include "tables.h"
uint32_t fz_add(uint32_t, uint32_t), fz_sub(uint32_t, uint32_t), fz_mul(uint32_t, uint32_t), fz_div(uint32_t, uint32_t), fz_from_i16(int16_t);
int16_t fz_to_i16(uint32_t);
int main(void)
{
    printf("static const uint32_t W[%d] = {", (int)(sizeof A / sizeof *A));
    for (unsigned i = 0; i < sizeof A / sizeof *A; i++) {
        uint32_t a = A[i], b = B[i], r;
        switch (OP) {
        case 0: r = fz_add(a, b); break;
        case 1: r = fz_sub(a, b); break;
        case 2: r = fz_mul(a, b); break;
        case 3: r = fz_div(a, b); break;
        case 4: r = fz_from_i16((int16_t)a); break;
        default: r = (uint32_t)(int32_t)fz_to_i16(a);
        }
        printf("%s0x%08Xu", i ? "," : "", r);
    }
    printf("};\n");
    return 0;
}
