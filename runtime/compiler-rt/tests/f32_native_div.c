/* Native x86-64: the llrm-cc-built sfb_divsf3 against the SSE divide on random normal operands. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
float sfb_divsf3(float, float);
static uint64_t s = 0x9E3779B97F4A7C15ull;
static uint64_t rnd(void) { s ^= s << 13; s ^= s >> 7; s ^= s << 17; return s; }
int main(void)
{
    long bad = 0;
    for (int i = 0; i < 200000; i++) {
        uint64_t r = rnd(), q = rnd(); uint32_t ua = (uint32_t)((r >> 63) << 31) | ((100 + (uint32_t)(r >> 40) % 51) << 23) | ((uint32_t)r & 0x7FFFFF), ub = (uint32_t)((q >> 63) << 31) | ((100 + (uint32_t)(q >> 40) % 51) << 23) | ((uint32_t)q & 0x7FFFFF);
        float a, b; memcpy(&a, &ua, 4); memcpy(&b, &ub, 4);
        volatile float vb = b; float want = a / vb, got = sfb_divsf3(a, b);
        if (memcmp(&want, &got, 4)) bad++;
    }
    printf("native f32 div mismatches: %ld\n", bad);
    return bad != 0;
}
