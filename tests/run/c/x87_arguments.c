// flags: -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv | -O1 -march=i486 -m32 | -O2 -march=i486
/* Float and double arguments to a call, mixed with integers: each goes into the space made for it on the stack, the order the callee reads. */
extern void report(long value);

static double mix(double a, float b, int n, double c, float d)
{
    if (n == 0) return a + b + c + d;
    return mix(a * 0.5 + 1.0, b + 1.0f, n - 1, c - a, d * 2.0f);
}

static long scaled(double a, float b, int n, double c, float d)
{
    return (long)(mix(a, b, n, c, d) * 1000.0 + 0.5);
}

int main(void)
{
    report(scaled(100.0, 3.0f, 0, 7.0, 0.25f));
    report(scaled(100.0, 3.0f, 5, 7.0, 0.25f));
    report(scaled(-8.5, 1.5f, 9, 2.0, 1.0f));
    return 0;
}
