/* Ours, one op per build: -DOP=0..5 (add sub mul div i2f f2i), -DNOCALL for the loop alone. Exit 0 when every result is bit-exact. */
#include <stdint.h>
#include "tables.h"
uint32_t __addsf3(uint32_t, uint32_t), __subsf3(uint32_t, uint32_t), __mulsf3(uint32_t, uint32_t), __divsf3(uint32_t, uint32_t);
uint32_t __floatsisf(int32_t);
int32_t __fixsfsi(uint32_t);
volatile uint32_t sink;
int main(void)
{
    int bad = 0;
    for (int i = 0; i < sizeof A / sizeof *A; i++) {
        uint32_t a = A[i], b = B[i], r;
#ifdef NOCALL
        r = W[i];
#elif OP == 0
        r = __addsf3(a, b);
#elif OP == 1
        r = __subsf3(a, b);
#elif OP == 2
        r = __mulsf3(a, b);
#elif OP == 3
        r = __divsf3(a, b);
#elif OP == 4
        r = __floatsisf((int32_t)a);
#else
        r = (uint32_t)__fixsfsi(a);
#endif
        sink = r;
        if (r != W[i]) bad++;
    }
    return bad > 255 ? 255 : bad;
}
