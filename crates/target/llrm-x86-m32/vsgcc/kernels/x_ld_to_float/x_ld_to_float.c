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

static float ld_to_float(LD v)
{
    return (float)v;
}

long bench_x_ld_to_float(int n)
{
    float t = 0;
    for (int r = 0; r < 4; r++) for (int i = 0; i < n; i++) t += ld_to_float(x[i] * 1.1L + r);
    return (long)(t * 16);
}

static void setup(void)
{
    init();
}

int main(void)
{
    setup();
    report(bench_x_ld_to_float(64));
    return 0;
}
