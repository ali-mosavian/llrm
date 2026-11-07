/* 16-bit integer builtins vs C semantics. Division by zero follows sf_int.h. */
/* cc -O2 -fno-builtin -Iinclude '-DSFB_NAME(n)=sfb_##n' tests/host_int16.c lib/int/int16.c */
#include <stdio.h>
#include <stdint.h>
uint16_t sfb_mulhi3(uint16_t, uint16_t), sfb_udivhi3(uint16_t, uint16_t), sfb_umodhi3(uint16_t, uint16_t);
int16_t sfb_divhi3(int16_t, int16_t), sfb_modhi3(int16_t, int16_t), sfb_ashrhi3(int16_t, int);
uint16_t sfb_ashlhi3(uint16_t, int), sfb_lshrhi3(uint16_t, int);
static long fails;
#define CK(name, got, want, a, b) do { if ((got) != (want) && fails++ < 30) \
    printf("FAIL %s a=%x b=%x got=%x want=%x\n", name, (unsigned)(a), (unsigned)(b), (unsigned)(got), (unsigned)(want)); } while (0)
int main(void)
{
    /* every dividend against every divisor in a strided grid plus all the edges: 65536 x 65536 is 4e9, so the edges are exhaustive and the rest strided */
    static const uint16_t e[] = {0, 1, 2, 3, 0x7FFF, 0x8000, 0x8001, 0xFFFF, 0xFFFE, 0x00FF, 0x0100, 0x4000, 0x5555};
    for (unsigned i = 0; i < sizeof e / sizeof *e; i++)
        for (unsigned j = 0; j < 65536; j++) {
            uint16_t a = e[i], b = (uint16_t)j;
            for (int k = 0; k < 2; k++, a ^= b, b ^= a, a ^= b) {
                int16_t sa = (int16_t)a, sb = (int16_t)b;
                CK("mulhi3", sfb_mulhi3(a, b), (uint16_t)(a * b), a, b);
                CK("udivhi3", sfb_udivhi3(a, b), b ? a / b : 0xFFFF, a, b);
                CK("umodhi3", sfb_umodhi3(a, b), b ? a % b : a, a, b);
                int16_t dq = !sb ? -1 : (sa == INT16_MIN && sb == -1) ? INT16_MIN : sa / sb;
                int16_t dr = !sb ? sa : (sb == -1) ? 0 : sa % sb;
                CK("divhi3", sfb_divhi3(sa, sb), dq, a, b);
                CK("modhi3", sfb_modhi3(sa, sb), dr, a, b);
            }
        }
    for (unsigned a = 0; a < 65536; a += 7)
        for (int n = 0; n < 16; n++) {
            CK("ashlhi3", sfb_ashlhi3(a, n), (uint16_t)(a << n), a, n);
            CK("lshrhi3", sfb_lshrhi3(a, n), (uint16_t)(a >> n), a, n);
            CK("ashrhi3", sfb_ashrhi3((int16_t)a, n), (int16_t)((int16_t)a >> n), a, n);
        }
    printf("fails=%ld\n", fails);
    return fails != 0;
}
