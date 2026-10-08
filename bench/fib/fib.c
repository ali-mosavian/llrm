// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

static int fib(int n)
{
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}

long bench_fib(int n)
{
    return fib(n);
}

int main(void)
{
    report(bench_fib(20));
    return 0;
}
