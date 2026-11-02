/* Callee-saved registers (d2-d7, a2-a6) survive every entry point of src/m68k/sf32.s on every path the operands reach.
 * Run: tests/m68k_asm.sh run 68000 1 tests/m68k_asm_regs.c */
#include <stdint.h>

float __addsf3(float, float), __subsf3(float, float), __mulsf3(float, float), __divsf3(float, float), __negsf2(float);
int __cmpsf2(float, float), __eqsf2(float, float), __nesf2(float, float), __ltsf2(float, float), __lesf2(float, float),
    __gesf2(float, float), __gtsf2(float, float), __unordsf2(float, float), __fixsfsi(float);
float __floatsisf(int32_t), __floatunsisf(uint32_t);
uint32_t __fixunssfsi(float);

/* h_canary(fn, a, b): calls fn(a, b) with d2-d7/a2-a6 set to known words; returns 0 if all came back. */
long h_canary(void *fn, uint32_t a, uint32_t b);
__asm__(".text\n.globl h_canary\nh_canary:\n"
        "\tmovem.l %d2-%d7/%a2-%a6,-(%sp)\n"
        "\tmove.l 48(%sp),%a0\n\tmove.l 56(%sp),-(%sp)\n\tmove.l 56(%sp),-(%sp)\n"
        "\tmove.l #0x22222222,%d2\n\tmove.l #0x33333333,%d3\n\tmove.l #0x44444444,%d4\n\tmove.l #0x55555555,%d5\n"
        "\tmove.l #0x66666666,%d6\n\tmove.l #0x77777777,%d7\n\tmove.l #0xA2A2A2A2,%a2\n\tmove.l #0xA3A3A3A3,%a3\n"
        "\tmove.l #0xA4A4A4A4,%a4\n\tmove.l #0xA5A5A5A5,%a5\n\tmove.l #0xA6A6A6A6,%a6\n"
        "\tjsr (%a0)\n\taddq.l #8,%sp\n\tmoveq #1,%d0\n"
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
    /* zeros, denormals (tiny and large), normals near and far, near-overflow, near-underflow, inf, NaN */
    static const uint32_t v[] = {0, 0x80000000, 1, 0x807FFFFF, 0x00400001, 0x3F800000, 0xBF800001, 0x3F800003, 0x4B000001,
                                 0x7F7FFFFF, 0xFF000000, 0x00800000, 0x0C000000, 0x33800000, 0x7F800000, 0xFF800000, 0x7FC00000, 0x7F800001};
    static void *const fn[] = {__addsf3, __subsf3, __mulsf3, __divsf3, __negsf2, __cmpsf2, __eqsf2, __nesf2,
                               __ltsf2, __lesf2, __gesf2, __gtsf2, __unordsf2,
                               __floatsisf, __floatunsisf, __fixsfsi, __fixunssfsi};
    const int nv = sizeof v / sizeof *v, nf = sizeof fn / sizeof *fn;
    int bad = 0;
    for (int f = 0; f < nf; f++)
        for (int i = 0; i < nv; i++)
            for (int j = 0; j < nv; j++) bad += h_canary(fn[f], v[i], v[j]) != 0;
    say(bad ? "FAILED\n" : "ALL OK\n");
    sys3(1, bad != 0, 0, 0);
    return bad;
}
void _start(void) { main(); for (;;) ; }
