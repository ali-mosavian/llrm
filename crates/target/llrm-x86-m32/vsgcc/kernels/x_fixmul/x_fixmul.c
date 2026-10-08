extern void report(long value);

static long fixmul(long a, long b) { return (long)(((long long)a * b) >> 16); }
long bench_x_fixmul(int n)
{
    int i; long x = 65536, sum = 0;
    for (i = 0; i < n; ++i) { x = fixmul(x, 65500 + (i & 31)) + (i & 7); sum += x >> 4; if (x > 4000000) x >>= 3; }
    return sum;
}

int main(void)
{
    report(bench_x_fixmul(1500));
    return 0;
}
