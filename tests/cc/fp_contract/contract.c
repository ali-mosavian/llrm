#include <stdio.h>
#include <string.h>
__attribute__((noinline)) double f(double a, double b, double c) { return a * b + c; }
__attribute__((noinline)) float g(float a, float b, float c) { return a * b + c; }
int main(void) {
    volatile double a = 1.0 + 0x1p-30, b = 1.0 + 0x1p-30, c = -1.0 - 0x1p-29;
    volatile float x = 1.0f + 0x1p-12f, y = 1.0f + 0x1p-12f, z = -1.0f - 0x1p-11f;
    double r = f(a, b, c); float s = g(x, y, z);
    unsigned long long u; unsigned v; memcpy(&u, &r, 8); memcpy(&v, &s, 4);
    printf("%016llx %08x\n", u, v);
    return 0;
}
