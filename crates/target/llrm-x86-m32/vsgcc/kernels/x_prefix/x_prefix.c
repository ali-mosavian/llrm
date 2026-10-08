extern void report(long value);

static int a[1024];
long bench_x_prefix(int n)
{
    int i, r; long sum = 0;
    for (i = 0; i < n; ++i) a[i] = (i * 5) & 15;
    for (r = 0; r < 4; ++r) { for (i = 1; i < n; ++i) a[i] += a[i - 1]; for (i = 0; i < n; ++i) a[i] &= 0xFFFF; }
    for (i = 0; i < n; ++i) sum += a[i];
    return sum;
}

int main(void)
{
    report(bench_x_prefix(600));
    return 0;
}
