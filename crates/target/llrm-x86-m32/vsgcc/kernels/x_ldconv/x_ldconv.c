extern void report(long value);

typedef long double LD;

static LD mix(int i, long l, unsigned u, double d, float f)
{
    return (LD)i + l + u + d + f;
}

long bench_x_ldconv(int n)
{
    long sum = 0;
    for (int i = 0; i < n; i++) {
        LD v = mix(i, i * 3L, (unsigned)i * 7u, i * 0.5, i * 0.25f);
        sum += (long)v + (int)(v * 0.5L) + (short)(v * 0.25L) + (long)(double)(v + 0.375L) + (long)(float)(v * 2);
        sum += (long)(unsigned)(v * 3);
    }
    return sum;
}

int main(void)
{
    report(bench_x_ldconv(1000));
    return 0;
}
