/* NaN to int: 0 by default (SF_FIX_NAN_UNDEF unset: every target but x86 m16); with SF_FIX_NAN_UNDEF the saturated value, as compiler-rt.
 * cc -O2 -Iinclude [-DSF_FIX_NAN_UNDEF] '-DSF_NAME(n)=sfb_##n' tests/host_fix_nan.c lib/fp/f32/fix_si.c lib/fp/f32/fix_usi.c && ./a.out */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "softfloat.h"
int32_t sfb_fixsfsi(sf_float);
uint32_t sfb_fixunssfsi(sf_float);
int main(void)
{
    uint32_t nans[] = { 0x7FC00000u, 0xFFC00000u, 0x7F800001u };
    int bad = 0;
    for (int i = 0; i < 3; i++) {
        sf_float a; memcpy(&a, &nans[i], 4);
#ifdef SF_FIX_NAN_UNDEF
        int32_t s = (nans[i] >> 31) ? INT32_MIN : INT32_MAX; uint32_t u = (nans[i] >> 31) ? 0 : UINT32_MAX; /* negative: 0, as every negative */
#else
        int32_t s = 0; uint32_t u = 0;
#endif
        if (sfb_fixsfsi(a) != s) { printf("fixsfsi(%08x) = %d\n", nans[i], sfb_fixsfsi(a)); bad++; }
        if (sfb_fixunssfsi(a) != u) { printf("fixunssfsi(%08x) = %u\n", nans[i], sfb_fixunssfsi(a)); bad++; }
    }
    printf("bad=%d\n", bad); return bad != 0;
}
