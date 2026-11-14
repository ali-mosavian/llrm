#include <stdio.h>
static unsigned char buf[200];
__attribute__((noinline)) void fwd(unsigned char *dst, const unsigned char *src, unsigned long n) {
    unsigned long i;
    for (i = 0; i < n; i++) dst[i] = src[i];
}
__attribute__((noinline)) void bwd(unsigned char *dst, const unsigned char *src, unsigned long n) {
    unsigned long i;
    for (i = n; i > 0; i--) dst[i - 1] = src[i - 1];
}
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
    printf("bad %d\n", bad);
}
