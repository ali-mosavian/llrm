/* Pinned rows for src/m68k/sf32.s, expected bits from the host FPU; one row per bug.
 * Run: tests/m68k_asm.sh run 68000 1 tests/m68k_asm_rows.c (and 68020, 68060) */
#include <stdint.h>

float __addsf3(float, float), __subsf3(float, float), __mulsf3(float, float), __divsf3(float, float);

static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static void say(const char *s) { int n = 0; while (s[n]) n++; sys3(4, 1, (long)s, n); }
static float b2f(uint32_t b) { union { uint32_t b; float f; } u; u.b = b; return u.f; }
static uint32_t f2b(float f) { union { uint32_t b; float f; } u; u.f = f; return u.b; }

static const struct { char op; uint32_t a, b, want; } rows[] = {
    /* mul: the carry's exponent increment was a word add, so e - 1 = -1 became 0xFFFF0000 and the pack added 0xFFFF
     * (got 0x00b40b83) */
    {'*', 0x94e9e962, 0xab43f3c7, 0x00b30b84},
    /* div, 68000 core: top halves of dividend and divisor equal (qhat = 0xFFFF); the remainder came from the swapped
     * dividend copy */
    {'/', 0x3F800001, 0x3F8000FF, 0x3f7ffe04},
    {'/', 0x3f9eec66, 0x401fe4fc, 0x3efe7200}, /* the same in the second digit */
    {'-', 0x80000001, 0x80000001, 0x00000000}, /* x - x is +0 (libgcc gives -0) */
    /* two NaNs: a's, quietened, as sf32.c and the host do; add returned the larger magnitude's (0xFFC00002) */
    {'+', 0x7FC00001, 0xFFC00002, 0x7FC00001},
    {'-', 0x7FC00001, 0xFFC00002, 0x7FC00001},
};

int main(void)
{
    int bad = 0;
    for (unsigned i = 0; i < sizeof rows / sizeof *rows; i++) {
        float x = b2f(rows[i].a), y = b2f(rows[i].b);
        float r = rows[i].op == '+' ? __addsf3(x, y) : rows[i].op == '-' ? __subsf3(x, y) : rows[i].op == '*' ? __mulsf3(x, y) : __divsf3(x, y);
        if (f2b(r) != rows[i].want) { bad++; say("MISMATCH row\n"); }
    }
    say(bad ? "FAILED\n" : "ALL OK\n");
    sys3(1, bad != 0, 0, 0);
    return bad;
}
void _start(void) { main(); for (;;) ; }
