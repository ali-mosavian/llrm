#include <stdio.h>
#include <math.h>
volatile double vd = -2.25; volatile float vf = 6.25f;
static double dmax(double a, double b) { return a > b ? a : b; }
static float fmin2(float a, float b) { return a < b ? a : b; }
int main(void) {
    double d = vd; float f = vf;
    printf("%ld %ld\n", (long)(-d * 100), (long)(fabs(d) * 100));
    printf("%ld %ld\n", (long)(-f * 100), (long)(fabsf(f) * 100));
    printf("%ld %ld\n", (long)(sqrt(-d * 4.0) * 1000), (long)(sqrtf(f) * 1000));
    printf("%ld %ld\n", (long)(dmax(d, 1.0) * 100), (long)(fmin2(f, 3.0f) * 100));
    double z = -0.0; printf("%d %d\n", z == 0.0, 1.0 / z < 0);
    return 0;
}
