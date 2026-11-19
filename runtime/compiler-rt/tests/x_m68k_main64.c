/* f64 twin of m68k_main.c for add/sub/mul/div: vectors64.h (gen_vectors64.c or gen_vectors64_edge.c), bit patterns compared.
 * A NaN result must carry sf64.c's payload: a quietened if a is NaN, else b (negated for sub) quietened, else 0x7FF80000_00000000. */
#include <stdint.h>
#include "vectors64.h"

double __adddf3(double, double), __subdf3(double, double), __muldf3(double, double), __divdf3(double, double);

static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static void say(const char *s) { int n = 0; while (s[n]) n++; sys3(4, 1, (long)s, n); }
static void hex(uint32_t v) { char b[10]; for (int i = 0; i < 8; i++) b[i] = "0123456789abcdef"[(v >> (28 - 4 * i)) & 15]; b[8] = ' '; b[9] = 0; say(b); }
static double mk(uint32_t hi, uint32_t lo) { union { uint32_t w[2]; double d; } u; u.w[0] = hi; u.w[1] = lo; return u.d; }
static void put(double d, uint32_t *hi, uint32_t *lo) { union { uint32_t w[2]; double d; } u; u.d = d; *hi = u.w[0]; *lo = u.w[1]; }
static int nan64(uint32_t hi, uint32_t lo) { return (hi & 0x7FFFFFFFu) > 0x7FF00000u || ((hi & 0x7FFFFFFFu) == 0x7FF00000u && lo); }

static const char *const oname[] = {"add", "sub", "mul", "div"};
static long bad, cnt;
static void chk(int op, const uint32_t *v, uint32_t gh, uint32_t gl, uint32_t wh, uint32_t wl)
{
    cnt++;
    if (nan64(wh, wl)) {
        if (nan64(v[0], v[1])) wh = v[0] | 0x80000u, wl = v[1];
        else if (nan64(v[2], v[3])) wh = (v[2] ^ (op == 1 ? 0x80000000u : 0)) | 0x80000u, wl = v[3];
        else wh = 0x7FF80000u, wl = 0;
    }
    if (gh == wh && gl == wl) return;
    if (bad++ < 12) { say(oname[op]); say(": a="); hex(v[0]); hex(v[1]); say("b="); hex(v[2]); hex(v[3]); say("got="); hex(gh); hex(gl); say("want="); hex(wh); hex(wl); say("\n"); }
}

int main(void)
{
    for (int i = 0; i < NV; i++) {
        const uint32_t *v = V[i];
        double x = mk(v[0], v[1]), y = mk(v[2], v[3]);
        uint32_t h, l;
        put(__adddf3(x, y), &h, &l); chk(0, v, h, l, v[4], v[5]);
        put(__subdf3(x, y), &h, &l); chk(1, v, h, l, v[6], v[7]);
        put(__muldf3(x, y), &h, &l); chk(2, v, h, l, v[8], v[9]);
        put(__divdf3(x, y), &h, &l); chk(3, v, h, l, v[10], v[11]);
    }
    say(bad ? "FAILED " : "ALL OK ");
    hex((uint32_t)NV);
    say("\n");
    return bad != 0;
}
void _start(void) { int r = main(); sys3(1, r, 0, 0); for (;;) ; }
