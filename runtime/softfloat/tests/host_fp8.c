/* Exhaustive check of src/sf8.c (all 65536 pairs per op) against double arithmetic, nearest even by search.
 * cc -O2 -Iinclude '-DSF8_NAME(n)=sf8b_##n' [-DFP8_M=3] tests/host_fp8.c src/sf8.c -lm */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#ifndef FP8_M
#define FP8_M 2
#endif
#define M FP8_M
#define E (7 - M)
#define BIAS ((1 << (E - 1)) - 1)
#define EMAX ((1 << E) - 1)
#define INF (EMAX << M)
typedef uint8_t fp8;
fp8 sf8b_add(fp8, fp8), sf8b_sub(fp8, fp8), sf8b_mul(fp8, fp8), sf8b_div(fp8, fp8);

static double val(int c)
{
    int e = (c >> M) & EMAX, m = c & ((1 << M) - 1);
    double v = e == EMAX ? (m ? NAN : INFINITY) : e == 0 ? ldexp(m, 1 - BIAS - M) : ldexp((1 << M) + m, e - BIAS - M);
    return c & 0x80 ? -v : v;
}
static int round8(double v, int neg)
{
    if (v == 0) return neg ? 0x80 : 0;
    double a = fabs(v);
    int best = 0;
    double bd = INFINITY;
    for (int c = 0; c <= INF; c++) {
        double x = c == INF ? ldexp(1, EMAX - BIAS) : val(c), d = fabs(x - a);
        if (d < bd || (d == bd && !(c & 1))) { bd = d; best = c; }
    }
    return best | (neg ? 0x80 : 0);
}
static int ref(int op, int a, int b)
{
    double x = val(a), y = val(b), r;
    if (isnan(x) || isnan(y)) return INF | (1 << (M - 1));
    int neg = ((a ^ b) & 0x80) != 0;
    switch (op) {
    case 0: r = x + y; if (isnan(r)) return INF | (1 << (M - 1));
        if (r == 0) return (x == 0 && y == 0 && (a & b & 0x80)) ? 0x80 : 0;
        if (isinf(r)) return r < 0 ? INF | 0x80 : INF; break;
    case 1: return ref(0, a, b ^ 0x80);
    case 2: r = x * y; neg = signbit(r) != 0; if (isnan(r)) return INF | (1 << (M - 1));
        if (isinf(r)) return neg ? INF | 0x80 : INF; neg = ((a ^ b) & 0x80) != 0; break;
    default: if (y == 0) return x == 0 ? INF | (1 << (M - 1)) : (neg ? INF | 0x80 : INF);
        r = x / y; if (isnan(r)) return INF | (1 << (M - 1));
        if (isinf(r)) return neg ? INF | 0x80 : INF; if (isinf(y)) return neg ? 0x80 : 0; neg = ((a ^ b) & 0x80) != 0; break;
    }
    return round8(r, op == 0 ? r < 0 : neg);
}
int main(void)
{
    static fp8 (*f[4])(fp8, fp8) = { sf8b_add, sf8b_sub, sf8b_mul, sf8b_div };
    static const char *name[4] = { "add", "sub", "mul", "div" };
    long bad = 0;
    for (int op = 0; op < 4; op++) {
        long fails = 0;
        for (int a = 0; a < 256; a++)
            for (int b = 0; b < 256; b++) {
                int want = ref(op, a, b), got = f[op](a, b);
                int nan = (want & 0x7F) > INF;
                if (nan ? (got & 0x7F) <= INF : got != want) { if (fails++ < 5) printf("FAIL %s %02x %02x got %02x want %02x\n", name[op], a, b, got, want); }
            }
        printf("E%dM%d %s pairs=65536 fails=%ld\n", E, M, name[op], fails);
        bad += fails;
    }
    return bad != 0;
}
