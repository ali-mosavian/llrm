/* Spike fast32 on the target: -DOP=0..5, -DNOCALL for the loop alone; the tables hold the host-computed results (W). */
#include <stdint.h>
#include "tables.h"
uint32_t fz_add(uint32_t, uint32_t), fz_sub(uint32_t, uint32_t), fz_mul(uint32_t, uint32_t), fz_div(uint32_t, uint32_t), fz_from_i16(int16_t);
int16_t fz_to_i16(uint32_t);
volatile uint32_t sink;
int main(void)
{
    int bad = 0;
    for (int i = 0; i < sizeof A / sizeof *A; i++) {
        uint32_t a = A[i], b = B[i], r;
#ifdef NOCALL
        r = W[i];
#elif OP == 0
        r = fz_add(a, b);
#elif OP == 1
        r = fz_sub(a, b);
#elif OP == 2
        r = fz_mul(a, b);
#elif OP == 3
        r = fz_div(a, b);
#elif OP == 4
        r = fz_from_i16((int16_t)a);
#else
        r = (uint32_t)(int32_t)fz_to_i16(a);
#endif
        sink = r;
        if (r != W[i]) bad++;
    }
    return bad > 255 ? 255 : bad;
}
