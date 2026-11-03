/* __fixsfsi and __fixunssfsi over all 2^32 inputs against the host: truncation in range, saturation outside, 0 for NaN (and for negatives, unsigned).
 * cc -O2 -Iinclude tests/fixsfsi_all.c src/sf32_conv.c -o /tmp/fixsfsi_all && /tmp/fixsfsi_all */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "softfloat.h"

int32_t __fixsfsi(float);
uint32_t __fixunssfsi(float);

int main(void)
{
    uint64_t bad = 0;
    uint32_t a = 0;
    do {
        float f;
        memcpy(&f, &a, 4);
        int32_t want = f != f ? 0 : f >= 2147483648.0f ? INT32_MAX : f <= -2147483648.0f ? INT32_MIN : (int32_t)f;
        uint32_t uwant = f != f || f < 1.0f ? 0 : f >= 4294967296.0f ? UINT32_MAX : (uint32_t)f;
        if (__fixsfsi(f) != want && bad++ < 5) printf("%08x: got %d want %d\n", a, (int)__fixsfsi(f), (int)want);
        if (__fixunssfsi(f) != uwant && bad++ < 5) printf("%08x: unsigned got %u want %u\n", a, (unsigned)__fixunssfsi(f), (unsigned)uwant);
    } while (++a);
    printf("%s: %llu differences over 2^32 inputs\n", bad ? "FAIL" : "ok", (unsigned long long)bad);
    return bad != 0;
}
