extern void report(long value);

long bench_x_primes(int n)
{
    int i, d, count = 0;
    for (i = 2; i < n; ++i) { for (d = 2; d * d <= i; ++d) if (i % d == 0) break; if (d * d > i) ++count; }
    return count;
}

int main(void)
{
    report(bench_x_primes(1200));
    return 0;
}
