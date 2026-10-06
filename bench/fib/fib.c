// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
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
