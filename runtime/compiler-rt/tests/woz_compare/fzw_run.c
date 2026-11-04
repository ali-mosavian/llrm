/* Host side of xcheck.py: reads "op x2 m2 x1 m1" lines (hex, the zero-page order of the original), prints the
 * translation's result as x1 m1 (8 hex digits), or "ovf" where the original would jump to its overflow vector. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
uint32_t fzw_add(uint32_t, uint32_t), fzw_sub(uint32_t, uint32_t), fzw_mul(uint32_t, uint32_t), fzw_div(uint32_t, uint32_t),
    fzw_from_i16(int16_t);
int16_t fzw_to_i16(uint32_t);
int main(void)
{
    char op[8];
    unsigned x2, m2, x1, m1;
    while (scanf("%7s %x %x %x %x", op, &x2, &m2, &x1, &m1) == 5) {
        uint32_t fp1 = (m1 << 8) | x1, fp2 = (m2 << 8) | x2, r;
        if (!strcmp(op, "add")) r = fzw_add(fp1, fp2);
        else if (!strcmp(op, "sub")) r = fzw_sub(fp2, fp1);
        else if (!strcmp(op, "mul")) r = fzw_mul(fp1, fp2);
        else if (!strcmp(op, "div")) r = fzw_div(fp2, fp1);
        else if (!strcmp(op, "i2f")) r = fzw_from_i16((int16_t)(m1 >> 8));
        else r = (uint32_t)(uint16_t)fzw_to_i16(fp1) << 16;   /* M1, M1+1 hold the integer */
        if (!strcmp(op, "f2i") && x1 > 0x8E) { printf("ovf\n"); continue; }   /* RTAR counts X1 through 255: the original jumps to its vector */
        if (r == 0x7FFFFFFFu) printf("ovf\n");
        else printf("%02x%06x\n", r & 255, (r >> 8) & 0xFFFFFF);
    }
    return 0;
}
