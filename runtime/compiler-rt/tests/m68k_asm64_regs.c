/* Callee-saved registers (d2-d7, a2-a6) survive add/sub/mul/div of config/m68k/sf64.s on every path the operands reach.
 * Run: tests/m68k_asm64.sh run 68000 1 tests/m68k_asm64_regs.c */
#include <stdint.h>

double __adddf3(double, double), __subdf3(double, double), __muldf3(double, double), __divdf3(double, double);

/* h_canary(fn, ah, al, bh, bl): calls fn(a, b) with d2-d7/a2-a6 set to known words; returns 0 if all came back. */
long h_canary(void *fn, uint32_t ah, uint32_t al, uint32_t bh, uint32_t bl);
__asm__(".text\n.globl h_canary\nh_canary:\n"
        "\tmovem.l %d2-%d7/%a2-%a6,-(%sp)\n"
        "\tmove.l 48(%sp),%a0\n\tmove.l 64(%sp),-(%sp)\n\tmove.l 64(%sp),-(%sp)\n\tmove.l 64(%sp),-(%sp)\n\tmove.l 64(%sp),-(%sp)\n"
        "\tmove.l #0x22222222,%d2\n\tmove.l #0x33333333,%d3\n\tmove.l #0x44444444,%d4\n\tmove.l #0x55555555,%d5\n"
        "\tmove.l #0x66666666,%d6\n\tmove.l #0x77777777,%d7\n\tmove.l #0xA2A2A2A2,%a2\n\tmove.l #0xA3A3A3A3,%a3\n"
        "\tmove.l #0xA4A4A4A4,%a4\n\tmove.l #0xA5A5A5A5,%a5\n\tmove.l #0xA6A6A6A6,%a6\n"
        "\tjsr (%a0)\n\tlea 16(%sp),%sp\n\tmoveq #1,%d0\n"
        "\tcmp.l #0x22222222,%d2\n\tbne 1f\n\tcmp.l #0x33333333,%d3\n\tbne 1f\n\tcmp.l #0x44444444,%d4\n\tbne 1f\n"
        "\tcmp.l #0x55555555,%d5\n\tbne 1f\n\tcmp.l #0x66666666,%d6\n\tbne 1f\n\tcmp.l #0x77777777,%d7\n\tbne 1f\n"
        "\tcmp.l #0xA2A2A2A2,%a2\n\tbne 1f\n\tcmp.l #0xA3A3A3A3,%a3\n\tbne 1f\n\tcmp.l #0xA4A4A4A4,%a4\n\tbne 1f\n"
        "\tcmp.l #0xA5A5A5A5,%a5\n\tbne 1f\n\tcmp.l #0xA6A6A6A6,%a6\n\tbne 1f\n\tmoveq #0,%d0\n"
        "1:\tmovem.l (%sp)+,%d2-%d7/%a2-%a6\n\trts\n");

static long sys3(long n, long a, long b, long c)
{
    register long d0 __asm__("d0") = n, d1 __asm__("d1") = a, d2 __asm__("d2") = b, d3 __asm__("d3") = c;
    __asm__ volatile("trap #0" : "+d"(d0) : "d"(d1), "d"(d2), "d"(d3) : "memory");
    return d0;
}
static void say(const char *s) { int n = 0; while (s[n]) n++; sys3(4, 1, (long)s, n); }

int main(void)
{
    /* zeros, denormals (tiny, one-word and large), normals near and far, near-overflow, near-underflow, inf, NaN */
    static const uint32_t v[][2] = {{0, 0}, {0x80000000, 0}, {0, 1}, {0x800FFFFF, 0xFFFFFFFF}, {0x00000001, 0x80000001}, {0x3FF00000, 0},
                                    {0xBFF00000, 1}, {0x3FF00000, 3}, {0x43300000, 1}, {0x7FEFFFFF, 0xFFFFFFFF}, {0xFFE00000, 0},
                                    {0x00100000, 0}, {0x01800000, 0}, {0x3CA00000, 0}, {0x7FF00000, 0}, {0xFFF00000, 0},
                                    {0x7FF80000, 0}, {0x7FF00000, 1}};
    static void *const fn[] = {__adddf3, __subdf3, __muldf3, __divdf3};
    const int nv = sizeof v / sizeof *v, nf = sizeof fn / sizeof *fn;
    int bad = 0;
    for (int f = 0; f < nf; f++)
        for (int i = 0; i < nv; i++)
            for (int j = 0; j < nv; j++) bad += h_canary(fn[f], v[i][0], v[i][1], v[j][0], v[j][1]) != 0;
    say(bad ? "FAILED\n" : "ALL OK\n");
    sys3(1, bad != 0, 0, 0);
    return bad;
}
void _start(void) { main(); for (;;) ; }
