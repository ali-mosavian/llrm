// flags: -O2 | -Os
/* `__builtin_rint` of a runtime value is a call to the C library's rint/rintf (here the program's own, half to even). */
extern void report(long value);
double rint(double x)
{
    double ax = x < 0 ? -x : x;
    long i = (long)ax;
    double f = ax - (double)i;
    if (f > 0.5 || (f == 0.5 && (i & 1))) ++i;
    return x < 0 ? -(double)i : (double)i;
}
float rintf(float x)
{
    float ax = x < 0 ? -x : x;
    long i = (long)ax;
    float f = ax - (float)i;
    if (f > 0.5f || (f == 0.5f && (i & 1))) ++i;
    return x < 0 ? -(float)i : (float)i;
}
volatile double v[6] = {2.5, 3.5, -2.5, 0.4, 7.0, -0.6};
int main(void)
{
    int k;
    for (k = 0; k < 6; ++k) {
        report((long)__builtin_rint(v[k]));
        report((long)__builtin_rintf((float)v[k]));
    }
    return 0;
}
