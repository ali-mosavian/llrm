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

static LD ld_from_ulong(unsigned long v)
{
    return (LD)v;
}

long bench_x_ld_from_ulong(int n)
{
    LD t = 0;
    for (int r = 0; r < 4; r++) for (int i = 0; i < n; i++) t += ld_from_ulong((unsigned long)i * 70000000UL + 5 + r);
    return (long)(t / 1000);
}

static void setup(void)
{
}

int main(void)
{
    setup();
    report(bench_x_ld_from_ulong(64));
    return 0;
}
