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

static LD ld_mix(int i, long l, unsigned u, double d, float f)
{
    return (LD)i + l + u + d + f;
}

long bench_x_ld_mix(int n)
{
    LD t = 0;
    for (int i = 0; i < n; i++) t += ld_mix(i, i * 3, 4000000000u + i, i * 0.5, i * 0.25f);
    return (long)(t / 1000);
}

static void setup(void)
{
}

int main(void)
{
    setup();
    report(bench_x_ld_mix(64));
    return 0;
}
