extern void report(long value);

static unsigned gcd(unsigned a, unsigned b) { while (b) { unsigned t = a % b; a = b; b = t; } return a; }
long bench_x_gcd(int n)
{
    unsigned i, j; long sum = 0;
    for (i = 1; i <= (unsigned)n / 20; ++i) for (j = 1; j <= (unsigned)n / 20; ++j) sum += gcd(i * 7, j * 11);
    return sum;
}

int main(void)
{
    report(bench_x_gcd(800));
    return 0;
}
