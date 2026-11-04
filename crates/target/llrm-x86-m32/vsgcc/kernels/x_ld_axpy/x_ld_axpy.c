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

static LD ld_axpy(LD a, LD *y0, const LD *x0, int n)
{
    for (int i = 0; i < n; i++) y0[i] += a * x0[i];
    return y0[0];
}

long bench_x_ld_axpy(int n)
{
    LD t = 0;
    for (int r = 0; r < 8; r++) t += ld_axpy(0.25L + r, y, x, n);
    return (long)(t + y[3] * 16);
}

static void setup(void)
{
    init();
}

int main(void)
{
    setup();
    report(bench_x_ld_axpy(64));
    return 0;
}
