/* usage: genm exp|log n -> ca65 vector records (a, 0, expected) from the C reference src/mathf.c on the host FPU.
 * cc -O2 -ffp-contract=off -fno-builtin -I../../include '-DSF_NAME(n)=sfb_##n' '-DSFM_NAME(n)=sfb_##n' genm.c ../../src/mathf.c ../../src/sf32_conv.c ../../src/sf32.c ../../src/sf32_conv64.c -lm */
#define SF_NAME(n) sfb_##n
#include "softfloat.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
float sfb_expf(float), sfb_logf(float);
static uint32_t rnd(void) { static uint64_t s = 88172645463325252ull; s ^= s << 13; s ^= s >> 7; s ^= s << 17; return (uint32_t)(s >> 16); }
static uint32_t bitsf(float f) { uint32_t b; memcpy(&b, &f, 4); return b; }
static float fbits(uint32_t b) { float f; memcpy(&f, &b, 4); return f; }
int main(int argc, char **argv)
{
    int isexp = argv[1][0] == 'e', n = atoi(argv[2]);
    static const uint32_t sp[] = {0, 0x80000000, 1, 0x007fffff, 0x00800000, 0x3f800000, 0xbf800000, 0x3f800001, 0x3f7fffff, 0x7f7fffff,
        0xff7fffff, 0x7f800000, 0xff800000, 0x7fc00000, 0x7fa00000, 0xffc00001, 0x42b17218, 0x42b17219, 0xc2cff1b5, 0xc2cff1b6,
        0x32000000, 0x31ffffff, 0xb2000000, 0x3fb504f3, 0x3fb504f4, 0x40000000, 0x3f000000, 0x4b000000, 0xc2aeac50, 0x42b00000};
    int ns = sizeof sp / sizeof *sp;
    printf("nrec = %d\nvec:\n", n);
    for (int i = 0; i < n; i++) {
        uint32_t a;
        if (i < ns) a = sp[i];
        else if (isexp) a = bitsf((float)((double)(rnd() % 4000000) / 4000000.0 * 200.0 - 105.0)), a = i % 7 == 0 ? (rnd() & 0x807fffff) | (uint32_t)(100 + rnd() % 30) << 23 : a;
        else { a = rnd() & 0x7fffffff; if (i % 5 == 0) a = bitsf(1.0f + (float)(rnd() % 2000 - 1000) / 1000.0f); if (i % 11 == 0) a &= 0x007fffff; }
        float r = isexp ? sfb_expf(fbits(a)) : sfb_logf(fbits(a));
        printf("  .dword $%08x, $00000000, $%08x\n", a, bitsf(r));
    }
    return 0;
}
