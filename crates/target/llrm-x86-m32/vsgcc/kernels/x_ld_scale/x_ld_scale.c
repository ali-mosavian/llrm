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

static LD ld_scale(LD *a, int n, LD f)
{
    for (int i = 0; i < n; i++) a[i] *= f;
    return f;
}

long bench_x_ld_scale(int n)
{
    LD t = 0;
    for (int r = 0; r < 8; r++) t += ld_scale(y, n, 1.25L + r * 0.01L);
    return (long)(t * 16 + y[5]);
}

static void setup(void)
{
    init();
}

int main(void)
{
    setup();
    report(bench_x_ld_scale(64));
    return 0;
}
