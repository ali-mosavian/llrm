// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

/* The accumulator is a parameter: a plain tail call. */
static long sum(long acc, int n)
{
    if (n == 0) return acc;
    return sum(acc + (n * 3) % 11, n - 1);
}

long bench_recsum(int n)
{
    return sum(0, n);
}

int main(void)
{
    report(bench_recsum(2000));
    return 0;
}
