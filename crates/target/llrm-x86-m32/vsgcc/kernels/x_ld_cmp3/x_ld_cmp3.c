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

static int ld_cmp3(LD a, LD b)
{
    return a < b ? -1 : (a > b);
}

long bench_x_ld_cmp3(int n)
{
    long t = 0;
    for (int r = 0; r < 4; r++) for (int i = 0; i < n; i++) t = t * 3 + ld_cmp3(x[i], x[(i * 7 + r) % 64]);
    return t;
}

static void setup(void)
{
    init();
}

int main(void)
{
    setup();
    report(bench_x_ld_cmp3(64));
    return 0;
}
