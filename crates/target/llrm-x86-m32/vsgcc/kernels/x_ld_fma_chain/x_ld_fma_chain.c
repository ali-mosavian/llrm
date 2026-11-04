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

static LD ld_fma_chain(const LD *a, const LD *b, const LD *c, int n)
{
    LD s = 0;
    for (int i = 0; i < n; i++) s += a[i] * b[i] + c[i];
    return s;
}

long bench_x_ld_fma_chain(int n)
{
    LD t = 0;
    for (int r = 0; r < 8; r++) { t += ld_fma_chain(x, y, z, n); z[r] += 1; }
    return (long)(t * 16);
}

static void setup(void)
{
    init();
}

int main(void)
{
    setup();
    report(bench_x_ld_fma_chain(64));
    return 0;
}
