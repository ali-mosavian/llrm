// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* fib(24) + sum of digit sums of 1..30000; recursion uses plain stack frames, no runtime frame helpers. */
extern void report(long value);

static long fib(int n)
{
    long a, b;
    if (n < 2)
        return n;
    a = fib(n - 1);
    b = fib(n - 2);
    return a + b;
}

static int digits(long v)
{
    int total = 0;
    long rest = v;
    while (rest > 0) {
        total += (int)(rest % 10);
        rest /= 10;
    }
    return total;
}

long bench_frames(void)
{
    long sum = 0, i;
    for (i = 1; i <= 30000; ++i)
        sum += digits(i);
    return fib(24) + sum;
}

int main(void)
{
    report(bench_frames());
    return 0;
}
