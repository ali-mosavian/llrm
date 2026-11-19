/* 6502 side of the vector run (SF_INTABI: floats are passed as their bit patterns). Exit code = mismatches (max 255). */
#include <stdint.h>
#include "vectors.h"

uint32_t __addsf3(uint32_t, uint32_t), __subsf3(uint32_t, uint32_t), __mulsf3(uint32_t, uint32_t), __divsf3(uint32_t, uint32_t);
int __cmpsf2(uint32_t, uint32_t), __unordsf2(uint32_t, uint32_t);
uint32_t __floatsisf(int32_t), __floatunsisf(uint32_t), __fixunssfsi(uint32_t);
int32_t __fixsfsi(uint32_t);

static int nan32(uint32_t b) { return (b << 1) > 0xFF000000u; }
static int same(uint32_t g, uint32_t w) { return g == w || (nan32(g) && nan32(w)); }

int main(void)
{
    int bad = 0;
    for (int i = 0; i < NV; i++) {
        const uint32_t *v = V[i];
        uint32_t a = v[0], b = v[1];
        if (!same(__addsf3(a, b), v[2])) bad++;
        if (!same(__subsf3(a, b), v[3])) bad++;
        if (!same(__mulsf3(a, b), v[4])) bad++;
        if (!same(__divsf3(a, b), v[5])) bad++;
        if ((__unordsf2(a, b) ? 2 : __cmpsf2(a, b)) != (int)v[6]) bad++;
        if (!same(__floatsisf((int32_t)a), v[7])) bad++;
        if (!same(__floatunsisf(a), v[8])) bad++;
        if (v[9] != 0xDEADBEEFu && (uint32_t)__fixsfsi(a) != v[9]) bad++;
        if (v[10] != 0xDEADBEEFu && __fixunssfsi(a) != v[10]) bad++;
    }
    return bad > 255 ? 255 : bad;
}
