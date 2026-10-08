extern void report(long value);

static double c[8] = {1.5, -0.25, 0.125, 2.0, -1.0, 0.5, 0.0625, 3.0};
long bench_x_horner(int n)
{
    int i, k; double sum = 0;
    for (i = 0; i < n; ++i) { double x = i * 0.01, p = 0; for (k = 0; k < 8; ++k) p = p * x + c[k]; sum += p; }
    return (long)(sum * 100);
}

int main(void)
{
    report(bench_x_horner(400));
    return 0;
}
