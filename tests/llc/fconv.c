#include <stdio.h>
#include <stdint.h>
volatile int32_t vi = -123456; volatile int64_t vl = 1234567890123LL; volatile uint32_t vu = 4000000000u;
volatile double vd = -1234.75; volatile float vf = 98765.5f;
int main(void) {
    int32_t i = vi; int64_t l = vl; uint32_t u = vu; double d = vd; float f = vf;
    printf("%lld %lld %lld\n", (long long)((double)i * 4), (long long)((double)l / 3), (long long)((double)u));
    volatile float lf = (float)l, q;  /* x87 keeps a float wider than 24 bits until it is stored */
    q = lf / 1000000;
    printf("%lld %lld %lld\n", (long long)((float)i), (long long)((float)u / 16), (long long)q);
    printf("%d %lld %lu\n", (int)d, (long long)(d * 1000000), (unsigned long)(uint32_t)(-d));
    printf("%ld %lld\n", (long)(int32_t)f, (long long)(f * 100));
    printf("%lld %lld\n", (long long)((double)f * 1000), (long long)((float)d * 1000));
    return 0;
}
