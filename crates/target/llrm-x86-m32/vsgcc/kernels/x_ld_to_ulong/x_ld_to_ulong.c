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

static unsigned long ld_to_ulong(LD v)
{
    return (unsigned long)v;
}

long bench_x_ld_to_ulong(int n)
{
    unsigned long t = 0;
    for (int r = 0; r < 4; r++) for (int i = 0; i < n; i++) t = t * 5 + (ld_to_ulong(x[i] + r * 67108864.0L) >> 24);
    return (long)t;
}

static void setup(void)
{
    for (int i = 0; i < 64; i++) x[i] = (i + 1) * 67108864.0L;
}

int main(void)
{
    setup();
    report(bench_x_ld_to_ulong(60));
    return 0;
}
