/* Like m68k_main.c, but no cap: counts mismatches per op and class of the expected result (vectors.h from gen_vectors.c).
 * Classes: zero sign (expected +-0, got the other zero), denormal (expected exponent 0, nonzero), special (inf/NaN), normal. */
#include <stdint.h>
#include "vectors.h"

float __addsf3(float, float), __subsf3(float, float), __mulsf3(float, float), __divsf3(float, float);
float __floatsisf(int32_t), __floatunsisf(uint32_t);
int32_t __fixsfsi(float);
uint32_t __fixunssfsi(float);

static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static void say(const char *s) { int n = 0; while (s[n]) n++; sys3(4, 1, (long)s, n); }
static void dec(long v) { char b[12]; int i = 11; b[i] = 0; b[--i] = ' '; do { b[--i] = '0' + v % 10; v /= 10; } while (v); say(b + i); }
static float b2f(uint32_t b) { union { uint32_t b; float f; } u; u.b = b; return u.f; }
static uint32_t f2b(float f) { union { uint32_t b; float f; } u; u.f = f; return u.b; }
static int nan32(uint32_t b) { return (b << 1) > 0xFF000000u; }

enum { ZERO, DEN, SPEC, NORM, NCLS };
static const char *const cname[] = {"zerosign", "denormal", "special", "normal"};
static long bad[6][NCLS], tot[6];
static const char *const oname[] = {"add", "sub", "mul", "div", "i2f/u2f", "f2i/f2u"};

static void chk(int op, uint32_t got, uint32_t want)
{
    tot[op]++;
    if (got == want || (nan32(got) && nan32(want))) return;
    uint32_t e = (want >> 23) & 255;
    int c = (want << 1) == 0 ? ZERO : e == 0 ? DEN : e == 255 ? SPEC : NORM;
    bad[op][c]++;
}

int main(void)
{
    for (int i = 0; i < NV; i++) {
        const uint32_t *v = V[i];
        float x = b2f(v[0]), y = b2f(v[1]);
        chk(0, f2b(__addsf3(x, y)), v[2]);
        chk(1, f2b(__subsf3(x, y)), v[3]);
        chk(2, f2b(__mulsf3(x, y)), v[4]);
        chk(3, f2b(__divsf3(x, y)), v[5]);
        chk(4, f2b(__floatsisf((int32_t)v[0])), v[7]);
        chk(4, f2b(__floatunsisf(v[0])), v[8]);
        if (v[9] != 0xDEADBEEFu) chk(5, (uint32_t)__fixsfsi(x), v[9]);
        if (v[10] != 0xDEADBEEFu) chk(5, __fixunssfsi(x), v[10]);
    }
    for (int o = 0; o < 6; o++) {
        say(oname[o]); say(" n="); dec(tot[o]);
        for (int c = 0; c < NCLS; c++) { say(cname[c]); say("="); dec(bad[o][c]); }
        say("\n");
    }
    return 0;
}
void _start(void) { int r = main(); sys3(1, r, 0, 0); for (;;) ; }
