extern void report(long value);

typedef long double LD;

static LD x[64], y[64], z[64];

static void init(void)
{
    for (int i = 0; i < 64; i++) {
        x[i] = i * 0.5L + 1;
        y[i] = 1.0L / (i + 1);
        z[i] = i;
    }
}

static LD ld_newton(LD a)
{
    LD x0 = a;
    for (int i = 0; i < 20; i++) x0 = (x0 + a / x0) * 0.5L;
    return x0;
}

long bench_x_ld_newton(int n)
{
    LD t = 0;
    for (int i = 0; i < n; i++) t += ld_newton(i + 2);
    return (long)(t * 16);
}

static void setup(void)
{
}

int main(void)
{
    setup();
    report(bench_x_ld_newton(8));
    return 0;
}
