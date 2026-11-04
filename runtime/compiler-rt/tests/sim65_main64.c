/* 6502 side of the f64 vector run (SF_INTABI: doubles are passed as their bit patterns). Builds on vectors64.h from
 * gen_vectors64[_edge].c -DEXT. Writes one line per op to outbuf (the _main wrapper in run_sim65_f64.sh prints it):
 * "<op> <class> <total> <bad>" per result class (zerosign, denormal, special, normal of the expected value), "cmp ..", "conv ..".
 * Exit code = mismatches (max 255). 64-bit values are handled as two 32-bit words so the harness itself uses no 64-bit operation. */
#include <stdint.h>
#include "vectors64.h"
#ifndef SHOW
#define SHOW 0
#endif

typedef union { uint32_t w[2]; uint64_t q; } D; /* w[0] = low word, w[1] = high word */
uint64_t __adddf3(uint64_t, uint64_t), __subdf3(uint64_t, uint64_t), __muldf3(uint64_t, uint64_t), __divdf3(uint64_t, uint64_t);
int __cmpdf2(uint64_t, uint64_t), __unorddf2(uint64_t, uint64_t), __eqdf2(uint64_t, uint64_t), __nedf2(uint64_t, uint64_t);
int __ltdf2(uint64_t, uint64_t), __ledf2(uint64_t, uint64_t), __gedf2(uint64_t, uint64_t), __gtdf2(uint64_t, uint64_t);
uint64_t __extendsfdf2(uint32_t);
uint32_t __truncdfsf2(uint64_t);
uint64_t __floatsidf(int32_t), __floatunsidf(uint32_t), __floatdidf(int64_t), __floatundidf(uint64_t);
int32_t __fixdfsi(uint64_t);
uint32_t __fixunsdfsi(uint64_t);
int64_t __fixdfdi(uint64_t);
uint64_t __fixunsdfdi(uint64_t);

char outbuf[1024];
unsigned outlen;

static void put(const char *s) { while (*s) outbuf[outlen++] = *s++; }
static void dec(unsigned v)
{
    char b[6]; int i = 5;
    b[i] = 0;
    do { b[--i] = '0' + v % 10; v /= 10; } while (v);
    put(b + i);
    put(" ");
}
static void hex(uint32_t v)
{
    for (int i = 0; i < 8; i++) outbuf[outlen++] = "0123456789abcdef"[(v >> (28 - 4 * i)) & 15];
    put(" ");
}

enum { ZERO, DEN, SPEC, NORM, NCLS };
static const char *const oname[] = {"add", "sub", "mul", "div"};
static const char *const cname[] = {"zerosign", "denormal", "special", "normal"};
static unsigned tot[4][NCLS], bad[4][NCLS];
static unsigned shown;

static D mk(uint32_t hi, uint32_t lo) { D d; d.w[0] = lo; d.w[1] = hi; return d; }
static int nan64(uint32_t hi, uint32_t lo) { return (hi & 0x7FF00000u) == 0x7FF00000u && ((hi & 0xFFFFFu) | lo); }

/* 1 if the 64-bit result g equals the expected (wh, wl), NaNs as a class. */
static int same(D g, uint32_t wh, uint32_t wl)
{
    return (g.w[1] == wh && g.w[0] == wl) || (nan64(g.w[1], g.w[0]) && nan64(wh, wl));
}

static void chk(int op, D g, const uint32_t *v, uint32_t wh, uint32_t wl)
{
    uint32_t e = (wh >> 20) & 0x7FF;
    int c = ((wh << 1) | wl) == 0 ? ZERO : e == 0 ? DEN : e == 0x7FF ? SPEC : NORM;
    tot[op][c]++;
    if (same(g, wh, wl)) return;
    bad[op][c]++;
    if (shown++ < SHOW) {
        put(oname[op]); put(": a="); hex(v[0]); hex(v[1]); put("b="); hex(v[2]); hex(v[3]);
        put("got="); hex(g.w[1]); hex(g.w[0]); put("want="); hex(wh); hex(wl); put("\n");
    }
}

static unsigned convn, convbad, cmpn, cmpbad;
static void cv(D g, uint32_t wh, uint32_t wl) { convn++; convbad += !(g.w[1] == wh && g.w[0] == wl); }
static void cv32(uint32_t g, uint32_t w) { convn++; convbad += g != w; }

int main(void)
{
    for (int i = 0; i < NV; i++) {
        const uint32_t *v = V[i];
        D x = mk(v[0], v[1]), y = mk(v[2], v[3]), r;
        r.q = __adddf3(x.q, y.q); chk(0, r, v, v[4], v[5]);
        r.q = __subdf3(x.q, y.q); chk(1, r, v, v[6], v[7]);
        r.q = __muldf3(x.q, y.q); chk(2, r, v, v[8], v[9]);
        r.q = __divdf3(x.q, y.q); chk(3, r, v, v[10], v[11]);
        int c = (int)v[12], un = c == 2;
        cmpn += 8;
        cmpbad += (__unorddf2(x.q, y.q) != 0) != un;
        cmpbad += (un ? 2 : __cmpdf2(x.q, y.q)) != c;
        cmpbad += (__eqdf2(x.q, y.q) == 0) != (c == 0);
        cmpbad += (__nedf2(x.q, y.q) != 0) != (c != 0);
        cmpbad += (__ltdf2(x.q, y.q) < 0) != (c == -1);
        cmpbad += (__ledf2(x.q, y.q) <= 0) != (c == -1 || c == 0);
        cmpbad += (__gedf2(x.q, y.q) >= 0) != (c == 0 || c == 1);
        cmpbad += (__gtdf2(x.q, y.q) > 0) != (c == 1);
        r.q = __floatsidf((int32_t)v[1]); cv(r, v[13], v[14]);
        r.q = __floatunsidf(v[1]); cv(r, v[16], v[17]);
        r.q = __floatdidf((int64_t)x.q); cv(r, v[18], v[19]);
        r.q = __floatundidf(x.q); cv(r, v[20], v[21]);
        cv32((uint32_t)__fixdfsi(x.q), v[22]);
        cv32(__fixunsdfsi(x.q), v[23]);
        r.q = (uint64_t)__fixdfdi(x.q); cv(r, v[24], v[25]);
        r.q = __fixunsdfdi(x.q); cv(r, v[26], v[27]);
        r.q = __extendsfdf2(v[1]); convn++; convbad += !same(r, v[28], v[29]);
        uint32_t t = __truncdfsf2(x.q); convn++; convbad += t != v[30] && !((t << 1) > 0xFF000000u && (v[30] << 1) > 0xFF000000u);
    }
    unsigned total = 0;
    for (int o = 0; o < 4; o++)
        for (int k = 0; k < NCLS; k++) {
            put(oname[o]); put(" "); put(cname[k]); put(" "); dec(tot[o][k]); dec(bad[o][k]); put("\n");
            total += bad[o][k];
        }
    put("cmp "); dec(cmpn); dec(cmpbad); put("\n");
    put("conv "); dec(convn); dec(convbad); put("\n");
    total += cmpbad + convbad;
    return total > 255 ? 255 : total;
}
