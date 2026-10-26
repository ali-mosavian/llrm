#include <stdio.h>
static double dsum(const double *a, int n) { double s = 0; for (int i = 0; i < n; i++) s += a[i]; return s; }
static float fsum(const float *a, int n) { float s = 0; for (int i = 0; i < n; i++) s += a[i]; return s; }
int main(void) {
    double d[16]; float f[16];
    for (int i = 0; i < 16; i++) { d[i] = i * 0.5 + 1.0; f[i] = i * 0.25f - 2.0f; }
    printf("%ld %ld\n", (long)(dsum(d, 16) * 1000), (long)(fsum(f, 16) * 1000));
    return 0;
}
