/* m68k soft-float smoke: binary64 add/sub/mul/div/compare/convert, binary32 add/mul, and 64-bit divide/shift, folded to one number the host reproduces (-DHOST). */
typedef unsigned long long u64; typedef long long i64;
#ifdef HOST
#include <stdio.h>
#define OUT(v) printf("%d\n", (int)(((v) ^ ((v) >> 8) ^ ((v) >> 16) ^ ((v) >> 24) ^ ((v) >> 32) ^ ((v) >> 40) ^ ((v) >> 48) ^ ((v) >> 56)) & 0xFF))
#else
#define OUT(v) ((void)0)
#endif
volatile double dv[6] = {1.0, -3.75, 1e-300, 123456789.125, 0.1, 1e300};
volatile float fv[4] = {1.5f, -2.25f, 3.0e-20f, 7.0f};
volatile i64 iv[4] = {1000000007LL, -123456789012345LL, 0x7FFFFFFFFFFFFFFFLL, 255};
volatile int sh[4] = {1, 13, 40, 63};
static u64 bits(double d) { union { double d; u64 u; } x; x.d = d; return x.u; }
static u64 bitsf(float f) { union { float f; unsigned u; } x; x.f = f; return x.u; }
int main(void) {
    u64 acc = 0; int i, j;
#ifndef SEC
#define SEC 7
#endif
    if (SEC & 1) for (i = 0; i < 6; i++) for (j = 0; j < 6; j++) {
        double a = dv[i], b = dv[j];
        acc = acc * 31 + bits(a + b); acc = acc * 31 + bits(a - b); acc = acc * 31 + bits(a * b); acc = acc * 31 + bits(a / b);
        acc = acc * 31 + (a < b) + 2 * (a == b) + 4 * (a > b);
    }
    if (SEC & 2) for (i = 0; i < 4; i++) for (j = 0; j < 4; j++) { acc = acc * 31 + bitsf(fv[i] + fv[j]); acc = acc * 31 + bitsf(fv[i] * fv[j]); }
    if (SEC & 4) for (i = 0; i < 4; i++) {
        acc = acc * 31 + bits((double)iv[i]); acc = acc * 31 + bitsf((float)iv[i]);
        acc = acc * 31 + (u64)(i64)dv[1 + (i & 1)];
        for (j = 0; j < 4; j++) { i64 d = iv[j] | 1; acc = acc * 31 + (u64)(iv[i] / d) + (u64)(iv[i] % d) + ((u64)iv[i] << sh[j]) + ((u64)iv[i] >> sh[j]) + (u64)(iv[i] >> sh[j]); }
    }
    OUT(acc);
    return (int)(acc ^ (acc >> 8) ^ (acc >> 16) ^ (acc >> 24) ^ (acc >> 32) ^ (acc >> 40) ^ (acc >> 48) ^ (acc >> 56)) & 0xFF;
}
