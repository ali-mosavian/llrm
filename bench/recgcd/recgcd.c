// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

static int gcd(int a, int b)
{
    if (b == 0) return a;
    return gcd(b, a % b);
}

long bench_recgcd(int n)
{
    long total = 0;
    int i;

    for (i = 1; i <= n; ++i) total += gcd(i * 7 + 3, i * 5 + 2) + gcd(i + 90, 360);
    return total;
}

int main(void)
{
    report(bench_recgcd(300));
    return 0;
}
