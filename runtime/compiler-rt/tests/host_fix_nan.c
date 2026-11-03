/* NaN to int: 0 by default (SF_FIX_NAN_UNDEF unset: every target but x86 m16); with SF_FIX_NAN_UNDEF the saturated value, as compiler-rt.
 * cc -O2 -Iinclude [-DSF_FIX_NAN_UNDEF] '-DSF_NAME(n)=sfb_##n' tests/host_fix_nan.c lib/fp/f32/fix_si.c lib/fp/f32/fix_usi.c lib/fp/f32/fix_di.c lib/fp/f64/fix.c lib/fp/f64/pack.c && ./a.out */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "softfloat.h"
int32_t sfb_fixsfsi(sf_float);
uint32_t sfb_fixunssfsi(sf_float);
int64_t sfb_fixsfdi(sf_float);
uint64_t sfb_fixunssfdi(sf_float);
int32_t sfb_fixdfsi(sf_double);
uint32_t sfb_fixunsdfsi(sf_double);
int64_t sfb_fixdfdi(sf_double);
uint64_t sfb_fixunsdfdi(sf_double);
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
    uint64_t dn[] = { 0x7FF8000000000000ull, 0xFFF8000000000000ull, 0x7FF0000000000001ull };
    for (int i = 0; i < 3; i++) {
        sf_double d; memcpy(&d, &dn[i], 8); int neg = dn[i] >> 63;
#ifdef SF_FIX_NAN_UNDEF
        int32_t s = neg ? INT32_MIN : INT32_MAX; uint32_t u = neg ? 0 : UINT32_MAX; int64_t s6 = neg ? INT64_MIN : INT64_MAX; uint64_t u6 = neg ? 0 : UINT64_MAX;
#else
        int32_t s = 0; uint32_t u = 0; int64_t s6 = 0; uint64_t u6 = 0;
#endif
        sf_float f; memcpy(&f, &nans[i], 4);
        if (sfb_fixdfsi(d) != s || sfb_fixunsdfsi(d) != u || sfb_fixdfdi(d) != s6 || sfb_fixunsdfdi(d) != u6) { printf("f64 fix of NaN %d wrong\n", i); bad++; }
        if (sfb_fixsfdi(f) != s6 || sfb_fixunssfdi(f) != u6) { printf("f32 fix to 64 of NaN %d wrong\n", i); bad++; }
    }
    printf("bad=%d\n", bad); return bad != 0;
}
