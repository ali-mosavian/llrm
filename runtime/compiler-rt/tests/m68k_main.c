/* Target side: calls the library's canonical names and compares bit patterns with vectors.h.
 * No float operation here, so the program touches nothing but the library under test.
 * Run: LLRM_CC_DIR=... tests/run_m68k.sh 68000 O2 */
#include <stdint.h>
#include "vectors.h"

#ifdef M68K_FLOAT_C /* the operations are tests/m68k_float_ops.c's plain float C, compiled by llrm-cc */
#define __addsf3 f_add
#define __subsf3 f_sub
#define __mulsf3 f_mul
#define __divsf3 f_div
#define __floatsisf f_i2f
#define __floatunsisf f_u2f
#define __fixsfsi f_f2i
#define __fixunssfsi f_f2u
int f_cmp(float, float);
#define CMP_(x, y) f_cmp(x, y)
#else
#define CMP_(x, y) (__unordsf2(x, y) ? 2 : __cmpsf2(x, y))
#endif

float __addsf3(float, float), __subsf3(float, float), __mulsf3(float, float), __divsf3(float, float);
int __cmpsf2(float, float), __unordsf2(float, float);
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
static void hex(uint32_t v) { char b[10]; for (int i = 0; i < 8; i++) b[i] = "0123456789abcdef"[(v >> (28 - 4 * i)) & 15]; b[8] = '\n'; b[9] = 0; say(b); }
static float b2f(uint32_t b) { union { uint32_t b; float f; } u; u.b = b; return u.f; }
static uint32_t f2b(float f) { union { uint32_t b; float f; } u; u.f = f; return u.b; }
static int nan32(uint32_t b) { return (b << 1) > 0xFF000000u; }
static int same(uint32_t g, uint32_t w) { return g == w || (nan32(g) && nan32(w)); }

static long bad, cnt[8];
static void chk(int op, const char *name, uint32_t a, uint32_t b, uint32_t got, uint32_t want)
{
    cnt[op]++;
    if (!same(got, want)) { if (bad++ < 12) { say("MISMATCH "); say(name); say(" a="); hex(a); say(" b="); hex(b); say(" got="); hex(got); say(" want="); hex(want); } }
}

int main(void)
{
    for (int i = 0; i < NV; i++) {
        const uint32_t *v = V[i];
        float x = b2f(v[0]), y = b2f(v[1]);
        chk(0, "add", v[0], v[1], f2b(__addsf3(x, y)), v[2]);
        chk(1, "sub", v[0], v[1], f2b(__subsf3(x, y)), v[3]);
        chk(2, "mul", v[0], v[1], f2b(__mulsf3(x, y)), v[4]);
        chk(3, "div", v[0], v[1], f2b(__divsf3(x, y)), v[5]);
        int c = CMP_(x, y);
        chk(4, "cmp", v[0], v[1], (uint32_t)c, (uint32_t)v[6]);
        chk(5, "i2f", v[0], 0, f2b(__floatsisf((int32_t)v[0])), v[7]);
        chk(5, "u2f", v[0], 0, f2b(__floatunsisf(v[0])), v[8]);
        if (v[9] != 0xDEADBEEFu) chk(6, "f2i", v[0], 0, (uint32_t)__fixsfsi(x), v[9]);
        if (v[10] != 0xDEADBEEFu) chk(6, "f2u", v[0], 0, __fixunssfsi(x), v[10]);
    }
    say(bad ? "FAILED\n" : "ALL OK\n");
    hex((uint32_t)NV);
    return bad != 0;
}
void _start(void) { int r = main(); sys3(1, r, 0, 0); for (;;) ; }
