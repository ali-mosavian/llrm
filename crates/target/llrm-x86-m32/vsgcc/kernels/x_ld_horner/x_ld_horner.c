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

static LD ld_horner(const LD *c, int n, LD x0)
{
    LD r = 0;
    for (int i = n - 1; i >= 0; i--) r = r * x0 + c[i];
    return r;
}

long bench_x_ld_horner(int n)
{
    LD t = 0;
    for (int r = 0; r < 8; r++) t += ld_horner(x, n, 1.0005L + r * 0.0001L);
    return (long)t;
}

static void setup(void)
{
    init();
}

int main(void)
{
    setup();
    report(bench_x_ld_horner(64));
    return 0;
}
