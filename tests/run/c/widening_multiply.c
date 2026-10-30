/* A product of two ints added to a long: the 64-bit multiply of the extended factors where the product
   cannot overflow, at the edges of what fits. */
#include <stdio.h>

__attribute__((noinline)) long acc(long s, int a, int b) { return s + a * b; }
__attribute__((noinline)) long sub(long s, int a, int b) { return s - a * b; }
__attribute__((noinline)) long alone(int a, int b) { return a * b; }
__attribute__((noinline)) unsigned long uacc(unsigned long s, unsigned a, unsigned b) { return s + (unsigned long)(a * b); }

int main(void) {
    int v[] = {0, 1, -1, 2, -2, 255, -256, 46340, -46340, 32767, -32768, 65535, 1 << 15, -(1 << 15)};
    long s = 5;
    unsigned long u = 7;
    for (unsigned i = 0; i < sizeof v / sizeof v[0]; i++)
        for (unsigned j = 0; j < sizeof v / sizeof v[0]; j++) {
            long p = (long)v[i] * v[j];
            if (p > 2147483647L || p < -2147483647L - 1) continue; /* overflow is undefined */
            s = acc(s, v[i], v[j]) ^ sub(s, v[j], v[i]) ^ alone(v[i], v[j]);
            u = uacc(u, (unsigned)v[i] & 0xffff, (unsigned)v[j] & 0xffff);
        }
    printf("%ld %lu\n", s, u);
    return 0;
}
