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

static LD ld_max(const LD *a, int n)
{
    LD m = a[0];
    for (int i = 1; i < n; i++) if (a[i] > m) m = a[i];
    return m;
}

long bench_x_ld_max(int n)
{
    LD t = 0;
    for (int r = 0; r < 8; r++) { t += ld_max(x, n); x[r] += 1; }
    return (long)(t * 16);
}

static void setup(void)
{
    init();
    for (int i = 0; i < 64; i++) x[i] = ((i * 37) % 64) * 0.5L;
}

int main(void)
{
    setup();
    report(bench_x_ld_max(64));
    return 0;
}
