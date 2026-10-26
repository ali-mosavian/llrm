#include <stdio.h>
volatile int vi = -123456; volatile long vl = 1234567890123L; volatile unsigned vu = 4000000000u;
volatile double vd = -1234.75; volatile float vf = 98765.5f;
int main(void) {
    int i = vi; long l = vl; unsigned u = vu; double d = vd; float f = vf;
    printf("%ld %ld %ld\n", (long)((double)i * 4), (long)((double)l / 3), (long)((double)u));
    printf("%ld %ld %ld\n", (long)((float)i), (long)((float)u / 16), (long)((float)l / 1000000));
    printf("%d %ld %u\n", (int)d, (long)(d * 1000000), (unsigned)(-d));
    printf("%d %ld\n", (int)f, (long)(f * 100));
    printf("%ld %ld\n", (long)((double)f * 1000), (long)((float)d * 1000));
    return 0;
}
