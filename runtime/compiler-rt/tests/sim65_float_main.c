/* The 6502 side of the vector run with plain float C: the compiler turns every operator into a call of the library (SoftenFloat), so this checks the
 * compiler-to-library contract end to end, where sim65_main.c calls the routines by name. Exit code = mismatches (max 255). */
#include <stdint.h>
#include "vectors.h"

static float bf(uint32_t b) { union { uint32_t u; float f; } x; x.u = b; return x.f; }
static uint32_t fb(float f) { union { uint32_t u; float f; } x; x.f = f; return x.u; }
static int nan32(uint32_t b) { return (b << 1) > 0xFF000000u; }
static int same(uint32_t g, uint32_t w) { return g == w || (nan32(g) && nan32(w)); }

int main(void)
{
    int bad = 0;
    for (int i = 0; i < NV; i++) {
        const uint32_t *v = V[i];
        uint32_t a = v[0], b = v[1];
        float x = bf(a), y = bf(b);
        if (!same(fb(x + y), v[2])) bad++;
        if (!same(fb(x - y), v[3])) bad++;
        if (!same(fb(x * y), v[4])) bad++;
        if (!same(fb(x / y), v[5])) bad++;
        if ((x != x || y != y ? 2 : x < y ? -1 : x > y ? 1 : 0) != (int)v[6]) bad++;
        if (!same(fb((float)(int32_t)a), v[7])) bad++;
        if (!same(fb((float)a), v[8])) bad++;
        if (v[9] != 0xDEADBEEFu && (uint32_t)(int32_t)x != v[9]) bad++;
        if (v[10] != 0xDEADBEEFu && (uint32_t)x != v[10]) bad++;
    }
    return bad > 255 ? 255 : bad;
}
