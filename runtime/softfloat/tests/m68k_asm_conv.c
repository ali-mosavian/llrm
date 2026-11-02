/* Float to int out of the vectors' range, per README: truncation, saturation, NaN gives 0, negative to unsigned gives 0.
 * Run: tests/m68k_asm.sh run 68000 1 tests/m68k_asm_conv.c */
#include <stdint.h>

int32_t __fixsfsi(float);
uint32_t __fixunssfsi(float);

static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static void say(const char *s) { int n = 0; while (s[n]) n++; sys3(4, 1, (long)s, n); }
static float b2f(uint32_t b) { union { uint32_t b; float f; } u; u.b = b; return u.f; }

int main(void)
{
    static const uint32_t s[][2] = {{0x4F000000, 0x7FFFFFFF}, {0x4EFFFFFF, 0x7FFFFF80}, {0xCF000000, 0x80000000}, {0xCF000001, 0x80000000},
        {0x7F800000, 0x7FFFFFFF}, {0xFF800000, 0x80000000}, {0x7FC00000, 0}, {0xFFC00000, 0}, {0x7F800001, 0}, {0xBF7FFFFF, 0},
        {0xBFC00000, 0xFFFFFFFF}, {0x80000001, 0}, {0x3F7FFFFF, 0}};
    static const uint32_t u[][2] = {{0x4F800000, 0xFFFFFFFF}, {0x4F7FFFFF, 0xFFFFFF00}, {0x7F800000, 0xFFFFFFFF}, {0x7FC00000, 0},
        {0x7F800001, 0}, {0xFFC00000, 0}, {0xBF800000, 0}, {0xBF000000, 0}, {0xFF800000, 0}, {0x3FC00000, 1}, {0x80000000, 0}};
    int bad = 0;
    for (unsigned i = 0; i < sizeof s / sizeof *s; i++) bad += (uint32_t)__fixsfsi(b2f(s[i][0])) != s[i][1];
    for (unsigned i = 0; i < sizeof u / sizeof *u; i++) bad += __fixunssfsi(b2f(u[i][0])) != u[i][1];
    say(bad ? "FAILED\n" : "ALL OK\n");
    sys3(1, bad != 0, 0, 0);
    return bad;
}
void _start(void) { main(); for (;;) ; }
