extern void report(long value);

long bench_x_ll_arith(int n)
{
    unsigned long long a = 0x123456789ABCDEFull, b = 7, sum = 0; int i;
    for (i = 0; i < n; ++i) { a = a * 6364136223846793005ull + 1442695040888963407ull; b = (a >> 33) | 1; sum += (a * b) ^ (a >> (i & 31)); sum += a << (i & 31); }
    return (long)(sum & 0x7FFFFFFF);
}

int main(void)
{
    report(bench_x_ll_arith(500));
    return 0;
}
