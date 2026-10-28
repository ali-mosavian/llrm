#include <stdio.h>
volatile int vi = 1; volatile double vd = 1.5; volatile float vf = 1.0f; volatile long vl = 4;
__attribute__((noinline)) static double mix(int a, double b, float c, long d, double e) { return a + b * 2 + c * 3 + d + e / 2; }
__attribute__((noinline)) static float nine(float a, float b, float c, float d, float e, float f, float g, float h, float i) { return a + b * 2 + c * 3 + d * 4 + e * 5 + f * 6 + g * 7 + h * 8 + i * 9; }
__attribute__((noinline)) static double dnine(double a, double b, double c, double d, double e, double f, double g, double h, double i, double j) { return a - b + c - d + e - f + g - h + i - j; }
__attribute__((noinline)) static float fret(int x) { return x * 0.5f; }
__attribute__((noinline)) static double live(double a, double b) { double t = a * b; double r = fret(vi + 2) + dnine(a, b, a, b, a, b, a, b, a, b); return t + r + vd; }
int main(void) {
    int i = vi; double d = vd; float f = vf; long l = vl;
    printf("%ld\n", (long)(mix(i, d + 1, f + 2.5f, l, d * 3.5) * 1000));
    printf("%ld\n", (long)(nine(f, f + 1, f + 2, f + 3, f + 4, f + 5, f + 6, f + 7, f + 8) * 1000));
    printf("%ld\n", (long)(dnine(d, d + 1, d + 2, d + 3, d + 4, d + 5, d + 6, d + 7, d + 8, d + 9) * 1000));
    printf("%ld\n", (long)(live(d, d + 2.5) * 1000));
    return 0;
}
