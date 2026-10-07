// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

/* The accumulating multiply, and a call that squares: halving exponents. */
static long power(long b, int e)
{
    if (e == 0) return 1;
    if (e & 1) return b * power(b, e - 1) % 30011;
    return power(b * b % 30011, e >> 1);
}

long bench_recpow(int n)
{
    long total = 0;
    int i;

    for (i = 1; i <= n; ++i) total += power(i % 97 + 2, i + 40) % 1000;
    return total;
}

int main(void)
{
    report(bench_recpow(400));
    return 0;
}
