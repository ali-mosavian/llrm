/* f64 <-> integer conversions: the xor-fold of the results over a fixed set; HOST builds it for the host FPU. */
typedef unsigned long long u64; typedef long long i64;
#ifdef HOST
typedef int i32; typedef unsigned u32;
#else
typedef long i32; typedef unsigned long u32; /* 32 bits on this target; int is 16 */
#endif
#ifndef HOST
u64 __floatsidf(i32); u64 __floatunsidf(u32); u64 __floatdidf(i64); u64 __floatundidf(u64);
u32 __floatsisf(i32); u32 __floatdisf(i64); i32 __fixsfsi(u32); i64 __fixsfdi(u32);
i32 __fixdfsi(u64); u32 __fixunsdfsi(u64); i64 __fixdfdi(u64); u64 __fixunsdfdi(u64);
#define FSI(x) __floatsidf(x)
#define FUSI(x) __floatunsidf(x)
#define FDI(x) __floatdidf(x)
#define FUDI(x) __floatundidf(x)
#define XSI(x) __fixdfsi(x)
#define XUSI(x) __fixunsdfsi(x)
#define XDI(x) __fixdfdi(x)
#define XUDI(x) __fixunsdfdi(x)
#define FSF(x) ((u64)__floatsisf(x))
#define FDF(x) ((u64)__floatdisf(x))
#define XSFSI(x) __fixsfsi(x)
#define XSFDI(x) __fixsfdi(x)
#else
#include <stdio.h>
#include <string.h>
static u64 b(double d) { u64 r; memcpy(&r, &d, 8); return r; }
static double d(u64 r) { double x; memcpy(&x, &r, 8); return x; }
#define FSI(x) b((double)(i32)(x))
#define FUSI(x) b((double)(u32)(x))
#define FDI(x) b((double)(i64)(x))
#define FUDI(x) b((double)(u64)(x))
#define XSI(x) ((i32)d(x))
#define XUSI(x) ((u32)d(x))
#define XDI(x) ((i64)d(x))
#define XUDI(x) ((u64)d(x))
static u32 fb(float f) { u32 r; memcpy(&r, &f, 4); return r; }
static float fd(u32 r) { float f; memcpy(&f, &r, 4); return f; }
#define FSF(x) ((u64)fb((float)(i32)(x)))
#define FDF(x) ((u64)fb((float)(i64)(x)))
#define XSFSI(x) ((i32)fd(x))
#define XSFDI(x) ((i64)fd(x))
#endif
static const i64 iv[] = {0, 1, -1, 2, 255, 65535, 65536, 0x7FFFFFFF, -0x7FFFFFFF - 1, 0x80000001LL, 0x123456789ABCDEFLL, 0x7FFFFFFFFFFFFFFFLL, -0x7FFFFFFFFFFFFFFFLL - 1, 0x0020000000000001LL, 0x00200000000000FFLL, -0x0020000000000003LL, 12345678901LL};
/* inputs valid for the C conversions they feed (out-of-range is undefined in C): small = all four, neg = signed only, big = 64-bit only */
static const u64 dsmall[] = {0x0000000000000000ull, 0x3FF0000000000000ull, 0x4059000000000000ull, 0x3FE0000000000000ull, 0x401C000000000000ull, 0x4170000000000000ull, 0x41DFFFFFFFC00000ull, 0x4000CCCCCCCCCCCDull};
static const u64 dneg[] = {0xBFF0000000000000ull, 0xC004000000000000ull, 0xC1E0000000000000ull, 0xBFE0000000000000ull};
static const u64 dbig[] = {0x41F0000000000000ull, 0x4202A05F20000000ull, 0x4330000000000001ull, 0x43DFFFFFFFFFFFFFull, 0x4340000000000000ull};
/* binary32 inputs, same classes: (s64)volatile float returned 0 in a library compiled before the call-frame fix */
static const u32 fsmall[] = {0x00000000ul, 0x3F800000ul, 0x42C80000ul, 0x3F000000ul, 0x40700000ul, 0x4B800000ul, 0x4EFFFFFFul, 0x3FC00000ul};
static const u32 fneg[] = {0xBF800000ul, 0xC0200000ul, 0xC0700000ul, 0xCF000000ul};
static const u32 fbig[] = {0x4F800000ul, 0x501502F9ul, 0x5EFFFFFFul, 0xDEFFFFFFul};
int main(void) {
    u64 acc[12] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}; unsigned i, k;
    for (i = 0; i < sizeof iv / sizeof iv[0]; i++) {
        acc[0] ^= FSI((i32)iv[i]); acc[1] ^= FUSI((u32)iv[i]); acc[2] ^= FDI(iv[i]); acc[3] ^= FUDI((u64)iv[i]); acc[8] ^= FSF((i32)iv[i]); acc[9] ^= FDF(iv[i]);
    }
    for (i = 0; i < sizeof dsmall / sizeof dsmall[0]; i++) {
        acc[4] ^= (u64)(i32)XSI(dsmall[i]); acc[5] ^= (u64)XUSI(dsmall[i]); acc[6] ^= (u64)XDI(dsmall[i]); acc[7] ^= (u64)XUDI(dsmall[i]);
    }
    for (i = 0; i < sizeof dneg / sizeof dneg[0]; i++) { acc[4] ^= (u64)(i32)XSI(dneg[i]); acc[6] ^= (u64)XDI(dneg[i]); }
    for (i = 0; i < sizeof dbig / sizeof dbig[0]; i++) { acc[6] ^= (u64)XDI(dbig[i]); acc[7] ^= (u64)XUDI(dbig[i]); }
    for (i = 0; i < sizeof fsmall / sizeof fsmall[0]; i++) { acc[10] ^= (u64)(i32)XSFSI(fsmall[i]); acc[11] ^= (u64)XSFDI(fsmall[i]); }
    for (i = 0; i < sizeof fneg / sizeof fneg[0]; i++) { acc[10] ^= (u64)(i32)XSFSI(fneg[i]); acc[11] ^= (u64)XSFDI(fneg[i]); }
    for (i = 0; i < sizeof fbig / sizeof fbig[0]; i++) acc[11] ^= (u64)XSFDI(fbig[i]);
#ifdef HOST
    for (k = 0; k < 12; k++) printf("%016llx%c", acc[k], k == 11 ? '\n' : ' ');
#else
    for (k = 0; k < 12; k++) ((volatile u64 *)0x8f00)[k] = acc[k];
#endif
    return 0;
}
