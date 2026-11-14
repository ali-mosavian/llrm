#include <stdio.h>
static unsigned char buf[200];
__attribute__((noinline)) int run_sum(const int *a, int n) { int t = 0; for (int i = 0; i < n; i++) t += a[i]; return t; }
__attribute__((noinline)) int run_xor(const int *a, int n) { int t = 0; for (int i = 0; i < n; i++) t ^= a[i] * 3; return t; }
__attribute__((noinline)) void fwd(unsigned char *dst, const unsigned char *src, unsigned long n) {
    unsigned long i;
    for (i = 0; i < n; i++) dst[i] = src[i];
}
__attribute__((noinline)) void bwd(unsigned char *dst, const unsigned char *src, unsigned long n) {
    unsigned long i;
    for (i = n; i > 0; i--) dst[i - 1] = src[i - 1];
}
void multiply_shorts(void);
extern short sa[1024], sb[1024];
void shift_ints(void);
extern int si[1024];
extern unsigned ui[1024];
int stack_copy(int n);
int square_sum(int n);
int main(void) {
    int bad = 0;
    for (int dir = 0; dir < 2; dir++)
    for (int shift = 1; shift <= 5; shift++)
    for (unsigned long n = 0; n < 100; n++) {
        unsigned char want[200], got[200];
        for (int i = 0; i < 200; i++) want[i] = got[i] = i * 13 + 5;
        if (dir == 0) { /* dst < src: forward */
            for (unsigned long i = 0; i < n; i++) want[8 + i] = want[8 + shift + i];
            fwd(got + 8, got + 8 + shift, n);
        } else {        /* dst > src: backward */
            for (unsigned long i = n; i > 0; i--) want[8 + shift + i - 1] = want[8 + i - 1];
            bwd(got + 8 + shift, got + 8, n);
        }
        for (int i = 0; i < 200; i++) if (got[i] != want[i]) { bad++; printf("dir %d shift %d n %lu at %d\n", dir, shift, n, i); break; }
    }
    static int ints[300];
    for (int i = 0; i < 300; i++) ints[i] = i * 7919 - 1000;
    for (int n = 0; n < 300; n++) {
        int s = 0, x = 0;
        for (int i = 0; i < n; i++) { s += ints[i]; x ^= ints[i] * 3; }
        if (run_sum(ints, n) != s || run_xor(ints, n) != x) bad++;
    }
    if (stack_copy(63) * 100 + stack_copy(5) != 303 || square_sum(5) != 30 || square_sum(50) != 40425) bad++;
    for (int i = 0; i < 1024; i++) { sa[i] = (short)(i * 31 - 7000); sb[i] = (short)(i * 17 + 3); }
    multiply_shorts();
    for (int i = 0; i < 1024; i++) if (sa[i] != (short)((short)(i * 31 - 7000) * (short)(i * 17 + 3))) bad++;
    for (int i = 0; i < 1024; i++) { si[i] = i * 100003 - 51200000; ui[i] = (unsigned)i * 2654435761u; }
    {
        static int want[1024];
        for (int i = 0; i < 1024; i++) want[i] = (int)((unsigned)si[i] << 3) + (si[i] >> 2) + (int)(ui[i] >> 5);
        shift_ints();
        for (int i = 0; i < 1024; i++) if (si[i] != want[i]) bad++;
    }
    printf("bad %d\n", bad);
}
