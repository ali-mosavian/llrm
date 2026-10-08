extern void report(long value);

static int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
long bench_x_fib(int n) { return fib(n); }

int main(void)
{
    report(bench_x_fib(20));
    return 0;
}
