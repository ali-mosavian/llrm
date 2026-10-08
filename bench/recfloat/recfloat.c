// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

static double decay(double x, double acc, int n)
{
    if (n == 0) return acc;
    return decay(x * 0.5 + 1.0, acc + x, n - 1);
}

long bench_recfloat(int n)
{
    return (long)(decay(100.0, 0.0, n) * 1000.0 + 0.5);
}

int main(void)
{
    report(bench_recfloat(1000));
    return 0;
}
