/* Every compare entry point against the compiler-rt contract: unordered is 1 for cmp/eq/ne/lt/le/unord-as-1 and -1 for ge/gt,
 * ordered operands give -1/0/1. Run: LLRM_CC_DIR=... m68k-run-c.sh 68020 Os tests/m68k_cmp_unordered.c src/sf32.c */
#include <stdint.h>

int __cmpsf2(float, float), __eqsf2(float, float), __nesf2(float, float), __ltsf2(float, float), __lesf2(float, float),
    __gesf2(float, float), __gtsf2(float, float), __unordsf2(float, float);

static float b2f(uint32_t b) { union { uint32_t b; float f; } u; u.b = b; return u.f; }
static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static void say(const char *s) { int n = 0; while (s[n]) n++; sys3(4, 1, (long)s, n); }

int main(void)
{
    static const uint32_t v[] = {0, 0x80000000, 0x3F800000, 0xBF800000, 0x7F800000, 0x00000001, 0x7FC00000, 0xFFC00001};
    int bad = 0;
    for (int i = 0; i < 8; i++)
        for (int j = 0; j < 8; j++) {
            float x = b2f(v[i]), y = b2f(v[j]);
            int nan = i >= 6 || j >= 6;
            int o = nan ? 0 : ((int32_t)v[i] < 0 ? -(int32_t)(v[i] & 0x7FFFFFFF) : (int32_t)v[i]) < ((int32_t)v[j] < 0 ? -(int32_t)(v[j] & 0x7FFFFFFF) : (int32_t)v[j]) ? -1
                : ((int32_t)v[i] < 0 ? -(int32_t)(v[i] & 0x7FFFFFFF) : (int32_t)v[i]) > ((int32_t)v[j] < 0 ? -(int32_t)(v[j] & 0x7FFFFFFF) : (int32_t)v[j]);
            int lo = nan ? 1 : o, hi = nan ? -1 : o;
            bad += __cmpsf2(x, y) != lo; bad += __eqsf2(x, y) != lo; bad += __nesf2(x, y) != lo;
            bad += __ltsf2(x, y) != lo; bad += __lesf2(x, y) != lo; bad += __gesf2(x, y) != hi; bad += __gtsf2(x, y) != hi;
            bad += __unordsf2(x, y) != nan;
        }
    say(bad ? "FAILED\n" : "ALL OK\n");
    sys3(1, bad != 0, 0, 0);
    return bad;
}
void _start(void) { main(); for (;;) ; }
