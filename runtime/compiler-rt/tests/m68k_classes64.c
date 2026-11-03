/* f64 twin of m68k_classes.c: vectors64.h from gen_vectors64.c, mismatches per op and class of the expected result. */
#include <stdint.h>
#include "vectors64.h"
#ifndef SHOW
#define SHOW 0
#endif

double __adddf3(double, double), __subdf3(double, double), __muldf3(double, double), __divdf3(double, double);
int __cmpdf2(double, double), __unorddf2(double, double);
double __floatsidf(int32_t);
int32_t __fixdfsi(double);

static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static void say(const char *s) { int n = 0; while (s[n]) n++; sys3(4, 1, (long)s, n); }
static void dec(long v) { char b[12]; int i = 11; b[i] = 0; b[--i] = ' '; do { b[--i] = '0' + v % 10; v /= 10; } while (v); say(b + i); }
static double mk(uint32_t hi, uint32_t lo) { union { uint32_t w[2]; double d; } u; u.w[0] = hi; u.w[1] = lo; return u.d; }
static void put(double d, uint32_t *hi, uint32_t *lo) { union { uint32_t w[2]; double d; } u; u.d = d; *hi = u.w[0]; *lo = u.w[1]; }

static const char *const oname[] = {"add", "sub", "mul", "div"};
enum { ZERO, DEN, SPEC, NORM, NCLS };
static const char *const cname[] = {"zerosign", "denormal", "special", "normal"};
static long bad[4][NCLS], tot[4];

static void hex(uint32_t v) { char b[10]; for (int i = 0; i < 8; i++) b[i] = "0123456789abcdef"[(v >> (28 - 4 * i)) & 15]; b[8] = ' '; b[9] = 0; say(b); }
static const uint32_t *cur;
static long shown;
static void chk(int op, uint32_t gh, uint32_t gl, uint32_t wh, uint32_t wl)
{
    tot[op]++;
    if (gh == wh && gl == wl) return;
    int gnan = (gh & 0x7FF00000u) == 0x7FF00000u && ((gh & 0xFFFFFu) | gl), wnan = (wh & 0x7FF00000u) == 0x7FF00000u && ((wh & 0xFFFFFu) | wl);
    if (gnan && wnan) return;
    uint32_t e = (wh >> 20) & 0x7FF;
    int c = ((wh << 1) | wl) == 0 ? ZERO : e == 0 ? DEN : e == 0x7FF ? SPEC : NORM;
    bad[op][c]++;
    if (SHOW && shown++ < SHOW) { say(oname[op]); say(": a="); hex(cur[0]); hex(cur[1]); say("b="); hex(cur[2]); hex(cur[3]); say("got="); hex(gh); hex(gl); say("want="); hex(wh); hex(wl); say("\n"); }
}

int main(void)
{
    long conv = 0, convbad = 0, cmpbad = 0;
    for (int i = 0; i < NV; i++) {
        const uint32_t *v = V[i];
        cur = v;
        double x = mk(v[0], v[1]), y = mk(v[2], v[3]);
        uint32_t h, l;
        put(__adddf3(x, y), &h, &l); chk(0, h, l, v[4], v[5]);
        put(__subdf3(x, y), &h, &l); chk(1, h, l, v[6], v[7]);
        put(__muldf3(x, y), &h, &l); chk(2, h, l, v[8], v[9]);
        put(__divdf3(x, y), &h, &l); chk(3, h, l, v[10], v[11]);
        int c = __unorddf2(x, y) ? 2 : __cmpdf2(x, y);
        cmpbad += c != (int)v[12];
        put(__floatsidf((int32_t)v[1]), &h, &l); conv++; convbad += h != v[13] || l != v[14];
        if (v[15] != 0xDEADBEEFu) { conv++; convbad += (uint32_t)__fixdfsi(x) != v[15]; }
    }
    for (int o = 0; o < 4; o++) {
        say(oname[o]); say(" n="); dec(tot[o]);
        for (int k = 0; k < NCLS; k++) { say(cname[k]); say("="); dec(bad[o][k]); }
        say("\n");
    }
    say("cmp bad="); dec(cmpbad); say("conv n="); dec(conv); say("bad="); dec(convbad); say("\n");
    return 0;
}
void _start(void) { int r = main(); sys3(1, r, 0, 0); for (;;) ; }
